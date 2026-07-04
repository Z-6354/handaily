//! 密码本：API 密钥加密存储
//!
//! - 可选主密码；未设置时使用 Windows DPAPI 保护本机主密钥
//! - 条目使用 AES-256-GCM 加密

use std::sync::Mutex;

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

const PBKDF2_ITERS: u32 = 120_000;
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;

/// 内存中的解锁状态（明文主密钥，进程退出即失）
pub struct VaultState {
    inner: Mutex<VaultInner>,
}

struct VaultInner {
    master_key: Option<[u8; KEY_LEN]>,
    has_password: bool,
    initialized: bool,
}

impl VaultState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(VaultInner {
                master_key: None,
                has_password: false,
                initialized: false,
            }),
        }
    }

    pub fn load_config(&self, db: &Connection) -> Result<(), String> {
        let has_pw = get_config_blob(db, "has_password")
            .map(|v| !v.is_empty() && v[0] == 1)
            .unwrap_or(false);
        let initialized = get_config_blob(db, "initialized")
            .map(|v| !v.is_empty() && v[0] == 1)
            .unwrap_or(false);

        let mut g = self.inner.lock().map_err(|e| e.to_string())?;
        g.has_password = has_pw;
        g.initialized = initialized;

        if initialized && !has_pw {
            g.master_key = Some(load_dpapi_master_key(db)?);
        }
        Ok(())
    }

    pub fn status(&self) -> VaultStatus {
        let g = self.inner.lock().unwrap();
        VaultStatus {
            initialized: g.initialized,
            has_password: g.has_password,
            unlocked: g.master_key.is_some(),
        }
    }

    pub fn is_unlocked(&self) -> bool {
        self.inner.lock().unwrap().master_key.is_some()
    }

    pub fn setup(&self, db: &Connection, password: Option<&str>) -> Result<(), String> {
        let mut g = self.inner.lock().map_err(|e| e.to_string())?;
        if g.initialized {
            return Err("密码本已初始化".into());
        }

        let key = if let Some(pw) = password.filter(|p| !p.is_empty()) {
            let salt = random_bytes(16);
            set_config_blob(db, "salt", &salt)?;
            let derived = derive_key(pw, &salt);
            let verifier = sha256_hex(&derived);
            set_config_blob(db, "password_verifier", verifier.as_bytes())?;
            set_config_blob(db, "has_password", &[1])?;
            g.has_password = true;
            derived
        } else {
            set_config_blob(db, "has_password", &[0])?;
            g.has_password = false;
            let key = random_key();
            save_dpapi_master_key(db, &key)?;
            key
        };

        set_config_blob(db, "initialized", &[1])?;
        g.initialized = true;
        g.master_key = Some(key);
        Ok(())
    }

    pub fn unlock(&self, db: &Connection, password: Option<&str>) -> Result<(), String> {
        let mut g = self.inner.lock().map_err(|e| e.to_string())?;
        if !g.initialized {
            return Err("请先初始化密码本".into());
        }
        if g.has_password {
            let pw = password.filter(|p| !p.is_empty()).ok_or("请输入密码")?;
            let salt = get_config_blob(db, "salt").ok_or("密码本配置损坏")?;
            let derived = derive_key(pw, &salt);
            let verifier = get_config_str(db, "password_verifier").ok_or("密码本配置损坏")?;
            if sha256_hex(&derived) != verifier {
                return Err("密码错误".into());
            }
            g.master_key = Some(derived);
        } else {
            g.master_key = Some(load_dpapi_master_key(db)?);
        }
        Ok(())
    }

    pub fn lock(&self) {
        if let Ok(mut g) = self.inner.lock() {
            g.master_key = None;
        }
    }

    pub fn list_entries(&self, db: &Connection) -> Result<Vec<VaultEntryMeta>, String> {
        if !self.is_unlocked() {
            return Err("请先解锁密码本".into());
        }
        list_entries(db)
    }

    pub fn add_entry(
        &self,
        db: &Connection,
        name: &str,
        website_url: &str,
        secret: &str,
    ) -> Result<i64, String> {
        let key = self.master_key()?;
        let (nonce, ciphertext) = encrypt(secret.as_bytes(), &key)?;
        let now = chrono::Local::now().to_rfc3339();
        db.execute(
            "INSERT INTO vault_entries (name, provider, note, nonce, ciphertext, created_at, updated_at) \
             VALUES (?1, '', ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![name, website_url, nonce, ciphertext, now, now],
        )
        .map_err(|e| e.to_string())?;
        Ok(db.last_insert_rowid())
    }

    pub fn update_entry(
        &self,
        db: &Connection,
        id: i64,
        name: &str,
        website_url: &str,
        secret: &str,
    ) -> Result<(), String> {
        let now = chrono::Local::now().to_rfc3339();
        if secret.is_empty() {
            db.execute(
                "UPDATE vault_entries SET name=?1, note=?2, updated_at=?3 WHERE id=?4",
                rusqlite::params![name, website_url, now, id],
            )
            .map_err(|e| e.to_string())?;
            return Ok(());
        }
        let key = self.master_key()?;
        let (nonce, ciphertext) = encrypt(secret.as_bytes(), &key)?;
        db.execute(
            "UPDATE vault_entries SET name=?1, note=?2, nonce=?3, ciphertext=?4, updated_at=?5 WHERE id=?6",
            rusqlite::params![name, website_url, nonce, ciphertext, now, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_entry(&self, db: &Connection, id: i64) -> Result<(), String> {
        db.execute("DELETE FROM vault_entries WHERE id = ?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_secret(&self, db: &Connection, id: i64) -> Result<String, String> {
        let row = db
            .query_row(
                "SELECT nonce, ciphertext FROM vault_entries WHERE id = ?1",
                [id],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .ok_or("条目不存在")?;
        let key = self.master_key()?;
        decrypt(&row.1, &row.0, &key)
    }

    fn master_key(&self) -> Result<[u8; KEY_LEN], String> {
        self.inner
            .lock()
            .map_err(|e| e.to_string())?
            .master_key
            .ok_or_else(|| "请先解锁密码本".into())
    }
}

#[derive(Debug, Serialize)]
pub struct VaultStatus {
    pub initialized: bool,
    pub has_password: bool,
    pub unlocked: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct VaultEntryMeta {
    pub id: i64,
    pub name: String,
    pub website_url: String,
    #[serde(skip_serializing)]
    pub nonce: Vec<u8>,
    #[serde(skip_serializing)]
    pub ciphertext: Vec<u8>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct VaultEntryInput {
    pub name: String,
    #[serde(default)]
    pub website_url: String,
    pub secret: String,
}

fn list_entries(db: &Connection) -> Result<Vec<VaultEntryMeta>, String> {
    let mut stmt = db
        .prepare(
            "SELECT id, name, note, nonce, ciphertext, created_at, updated_at \
             FROM vault_entries ORDER BY updated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(VaultEntryMeta {
                id: row.get(0)?,
                name: row.get(1)?,
                website_url: row.get(2)?,
                nonce: row.get(3)?,
                ciphertext: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn derive_key(password: &str, salt: &[u8]) -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERS, &mut key);
    key
}

fn random_key() -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut key);
    key
}

fn random_bytes(n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    OsRng.fill_bytes(&mut buf);
    buf
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    let hash = Sha256::digest(data);
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}

fn encrypt(plaintext: &[u8], key: &[u8; KEY_LEN]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|e| e.to_string())?;
    Ok((nonce.to_vec(), ct))
}

fn decrypt(ciphertext: &[u8], nonce: &[u8], key: &[u8; KEY_LEN]) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let pt = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| "解密失败".to_string())?;
    String::from_utf8(pt).map_err(|_| "解密数据无效".into())
}

fn get_config_blob(db: &Connection, key: &str) -> Option<Vec<u8>> {
    db.query_row(
        "SELECT value FROM vault_config WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .ok()
}

fn get_config_str(db: &Connection, key: &str) -> Option<String> {
    get_config_blob(db, key).map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn set_config_blob(db: &Connection, key: &str, value: &[u8]) -> Result<(), String> {
    db.execute(
        "INSERT INTO vault_config (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn save_dpapi_master_key(db: &Connection, key: &[u8; KEY_LEN]) -> Result<(), String> {
    let protected = dpapi_protect(key)?;
    set_config_blob(db, "dpapi_master_key", &protected)
}

fn load_dpapi_master_key(db: &Connection) -> Result<[u8; KEY_LEN], String> {
    let blob = get_config_blob(db, "dpapi_master_key").ok_or("未找到本机主密钥")?;
    let plain = dpapi_unprotect(&blob)?;
    if plain.len() != KEY_LEN {
        return Err("主密钥长度无效".into());
    }
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&plain);
    Ok(key)
}

#[cfg(windows)]
fn dpapi_protect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::LocalFree;
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_LOCAL_MACHINE,
    };

    unsafe {
        let mut in_blob = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(
            &mut in_blob,
            PWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_LOCAL_MACHINE,
            &mut out_blob,
        )
        .map_err(|e| format!("DPAPI 加密失败: {e}"))?;
        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
        let result = slice.to_vec();
        let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(out_blob.pbData as _)));
        Ok(result)
    }
}

#[cfg(windows)]
fn dpapi_unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Foundation::LocalFree;
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    unsafe {
        let mut in_blob = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(&mut in_blob, None, None, None, None, 0, &mut out_blob)
            .map_err(|e| format!("DPAPI 解密失败: {e}"))?;
        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
        let result = slice.to_vec();
        let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(out_blob.pbData as _)));
        Ok(result)
    }
}

#[cfg(not(windows))]
fn dpapi_protect(_data: &[u8]) -> Result<Vec<u8>, String> {
    Err("无密码模式仅支持 Windows".into())
}

#[cfg(not(windows))]
fn dpapi_unprotect(_data: &[u8]) -> Result<Vec<u8>, String> {
    Err("无密码模式仅支持 Windows".into())
}
