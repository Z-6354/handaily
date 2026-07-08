const UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 HANDAILY-blhx-mcp/1.0";

async function fetchJson(url) {
  const res = await fetch(url, { headers: { "User-Agent": UA } });
  if (!res.ok) throw new Error(`${res.status} ${url}`);
  return res.json();
}

const catalog = await fetchJson(
  "https://wiki.biligame.com/blhx/api.php?action=parse&page=%E8%88%B0%E8%88%B9%E5%9B%BE%E9%89%B4&prop=text&format=json"
);
const catalogHtml = catalog.parse.text["*"];
const shipLinks = [
  ...catalogHtml.matchAll(/href="\/blhx\/([^"#?]+)"/g),
].map((m) => decodeURIComponent(m[1]));
const unique = [...new Set(shipLinks)].filter(
  (t) => !t.startsWith("分类:") && !t.startsWith("File:") && !t.includes(":")
);
console.log("catalog links", unique.length, unique.slice(0, 10));

const ship = await fetchJson(
  "https://wiki.biligame.com/blhx/api.php?action=parse&page=%E6%AC%A7%E6%A0%B9%E4%BA%B2%E7%8E%8B&prop=text&format=json"
);
const html = ship.parse.text["*"];
const keys = [...new Set([...html.matchAll(/data-key="([^"]+)"/g)].map((m) => m[1]))];
console.log("line keys sample", keys.slice(0, 20), "total", keys.length);
const imgs = [...html.matchAll(/src="(https:\/\/patchwiki\.biligame\.com[^"]+)"/g)].map((m) => m[1]);
console.log("images", imgs.length, imgs.slice(0, 3));
