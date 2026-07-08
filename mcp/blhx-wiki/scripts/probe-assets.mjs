import { parseShipPage } from "../dist/scraper.js";
import { fetchWikiHtml } from "../dist/wiki.js";

const html = await fetchWikiHtml("欧根亲王");
const record = parseShipPage(html, "欧根亲王", "https://wiki.biligame.com/blhx/欧根亲王");
console.log("assets", record.assets.length, record.assets.slice(0, 5));
const imgCount = [...html.matchAll(/patchwiki\.biligame\.com\/images\/blhx/g)].length;
console.log("raw img refs", imgCount);
