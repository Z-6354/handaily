package com.handaily.hantransfer

import android.app.Activity
import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.provider.DocumentsContract
import android.util.Log
import android.view.Gravity
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.TextView

class MainActivity : Activity() {
    private lateinit var webView: WebView
    private lateinit var bridge: HantransferBridge
    private var fileChooserCallback: ValueCallback<Array<Uri>>? = null

    @Suppress("DEPRECATION")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == REQ_FILE) {
            val callback = fileChooserCallback
            fileChooserCallback = null
            if (resultCode != RESULT_OK || data == null) {
                callback?.onReceiveValue(null)
                return
            }
            val uris = mutableListOf<Uri>()
            val clip = data.clipData
            if (clip != null && clip.itemCount > 0) {
                for (i in 0 until clip.itemCount) uris += clip.getItemAt(i).uri
            } else {
                data.data?.let { uris += it }
            }
            callback?.onReceiveValue(uris.toTypedArray())
            if (::bridge.isInitialized && uris.isNotEmpty()) {
                bridge.onFilesPicked(uris)
            }
            return
        }
    }

    override fun onResume() {
        super.onResume()
        if (::bridge.isInitialized) {
            bridge.onResume()
        }
    }

    @android.annotation.SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: android.os.Bundle?) {
        super.onCreate(savedInstanceState)
        try {
            setupWebView()
        } catch (e: Exception) {
            Log.e(TAG, "onCreate failed", e)
            showFatalError(e)
        }
    }

    private fun setupWebView() {
        webView = WebView(this).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.allowFileAccess = true
            settings.databaseEnabled = true
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
                settings.mixedContentMode = WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
            }
        }

        bridge = HantransferBridge(this) { js ->
            runOnUiThread {
                if (::webView.isInitialized) {
                    webView.evaluateJavascript(js, null)
                }
            }
        }

        webView.addJavascriptInterface(bridge, "Hantransfer")
        webView.webChromeClient = object : WebChromeClient() {
            override fun onShowFileChooser(
                webView: WebView?,
                filePathCallback: ValueCallback<Array<Uri>>?,
                fileChooserParams: FileChooserParams?,
            ): Boolean {
                fileChooserCallback?.onReceiveValue(null)
                fileChooserCallback = filePathCallback
                launchFilePicker()
                return true
            }
        }
        webView.webViewClient = object : WebViewClient() {
            override fun onPageFinished(view: WebView?, url: String?) {
                if (url?.startsWith("file:") == true) {
                    runCatching { bridge.startDiscovery() }
                }
                webView.evaluateJavascript(
                    "window.__markNativeApp && window.__markNativeApp()",
                    null,
                )
            }

            override fun onReceivedError(
                view: WebView?,
                request: WebResourceRequest?,
                error: WebResourceError?,
            ) {
                if (request?.isForMainFrame == true) {
                    Log.e(TAG, "WebView error: ${error?.description}")
                }
            }

            @Deprecated("Deprecated in Java")
            override fun onReceivedError(view: WebView?, errorCode: Int, description: String?, failingUrl: String?) {
                Log.e(TAG, "WebView error: $description url=$failingUrl")
            }
        }
        // Always boot from bundled assets so JS bridge hooks stay in sync with the APK.
        webView.loadUrl("file:///android_asset/mobile/index.html")
        setContentView(webView)
        bridge.onStartup()
    }

    fun webViewUrl(): String? = if (::webView.isInitialized) webView.url else null

    private fun showFatalError(error: Exception) {
        val tv = TextView(this).apply {
            text = "hantransfer 启动失败\n\n${error.message ?: error.javaClass.simpleName}"
            setTextColor(Color.parseColor("#991B1B"))
            gravity = Gravity.CENTER
            setPadding(48, 48, 48, 48)
        }
        setContentView(tv)
    }

    @Suppress("DEPRECATION")
    fun launchFilePicker() {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = "*/*"
            putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
            putExtra(Intent.EXTRA_MIME_TYPES, arrayOf("*/*", "application/octet-stream"))
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                StorageAccess.buildPrimaryStorageUri()?.let { initial ->
                    putExtra(DocumentsContract.EXTRA_INITIAL_URI, initial)
                }
            }
        }
        startActivityForResult(intent, REQ_FILE)
    }

    override fun onDestroy() {
        if (::bridge.isInitialized) {
            bridge.onDestroy()
        }
        if (::webView.isInitialized) {
            webView.destroy()
        }
        super.onDestroy()
    }

    companion object {
        private const val TAG = "HantransferApp"
        private const val REQ_FILE = 1001
    }
}
