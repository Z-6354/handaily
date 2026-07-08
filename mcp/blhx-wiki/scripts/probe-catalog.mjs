const UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 HANDAILY-blhx-mcp/1.0";

const res = await fetch(
  "https://wiki.biligame.com/blhx/api.php?action=parse&page=%E8%88%B0%E8%88%B9%E5%9B%BE%E9%89%B4&prop=text&format=json",
  { headers: { "User-Agent": UA } }
);
const html = (await res.json()).parse.text["*"];

const avatars = [
  ...html.matchAll(
    /(?:alt|title)="([^"]+?)头像\.jpg"[^>]*>[\s\S]{0,400}?>([^<]+)</g
  ),
];
console.log("avatar pattern matches", avatars.length);

const cards = [
  ...html.matchAll(
    /href="\/blhx\/([^"#?]+)"[^>]*class="[^"]*cardSelect[^"]*"[^>]*>([\s\S]{0,200}?)<\/a>/g
  ),
];
console.log("cardSelect", cards.length);

const titles = [
  ...html.matchAll(/\/blhx\/([^"#?]+)"[^>]*title="([^"]+)"/g),
];
const shipTitles = titles.filter(([, t]) => !t.includes(":"));
console.log("title links", shipTitles.length, shipTitles.slice(0, 5));

const avatarNames = [
  ...html.matchAll(/([\u4e00-\u9fffA-Za-z0-9·\.\(\)（）μ]+)头像\.jpg/g),
].map((m) => m[1]);
const uniqueAvatars = [...new Set(avatarNames)];
console.log("avatar file names", uniqueAvatars.length, uniqueAvatars.slice(0, 15));

const cardDataMatch = html.match(/CardSelectTr\.init\(([\s\S]+?)\);/);
console.log("CardSelectTr.init", cardDataMatch ? cardDataMatch[1].slice(0, 200) : "none");

const dataList = html.match(/var\s+cardSelectData\s*=\s*(\[[\s\S]+?\]);/);
console.log("cardSelectData", dataList ? dataList[1].slice(0, 300) : "none");
