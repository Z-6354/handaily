const UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 HANDAILY-blhx-mcp/1.0";

const res = await fetch(
  "https://wiki.biligame.com/blhx/api.php?action=parse&page=%E8%88%B0%E8%88%B9%E5%9B%BE%E9%89%B4&prop=text&format=json",
  { headers: { "User-Agent": UA } }
);
const html = (await res.json()).parse.text["*"];
const idx = html.indexOf('id="CardSelectTr"');
console.log("CardSelectTr snippet:\n", html.slice(idx, idx + 2500));
