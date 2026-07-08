const UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 HANDAILY-blhx-wiki/1.0";

async function tryFetch(url, headers = {}) {
  const res = await fetch(url, {
    headers: {
      "User-Agent": UA,
      Accept: "application/json,text/html,*/*",
      "Accept-Language": "zh-CN,zh;q=0.9",
      Referer: "https://wiki.biligame.com/blhx/",
      ...headers,
    },
  });
  return { status: res.status, text: await res.text() };
}

const api =
  "https://wiki.biligame.com/blhx/api.php?action=parse&page=%E8%88%B0%E8%88%B9%E5%9B%BE%E9%89%B4&prop=text&format=json";
const page = "https://wiki.biligame.com/blhx/%E8%88%B0%E8%88%B9%E5%9B%BE%E9%89%B4";

console.log("api", await tryFetch(api));
console.log("page", (await tryFetch(page)).status, (await tryFetch(page)).text.slice(0, 200));
