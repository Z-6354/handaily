const UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 HANDAILY-blhx-mcp/1.0";

const res = await fetch(
  "https://wiki.biligame.com/blhx/api.php?action=parse&page=%E6%AC%A7%E6%A0%B9%E4%BA%B2%E7%8E%8B&prop=text&format=json",
  { headers: { "User-Agent": UA } }
);
const html = (await res.json()).parse.text["*"];
const sectionStart = html.indexOf('id="舰船台词"');
const section = html.slice(sectionStart, sectionStart + 8000);
const audio = [...section.matchAll(/href="([^"]+\.(?:ogg|mp3|wav))"/gi)].map((m) => m[1]);
console.log("audio links", audio.slice(0, 5));
const block = section.match(/ship_word_block[\s\S]{0,1500}/);
console.log(block?.[0]?.slice(0, 1200));
