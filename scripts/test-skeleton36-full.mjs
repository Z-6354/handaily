/**
 * Full parse test for chaijun.skel via SkeletonBinary36 + real atlas.
 */
import { build } from "esbuild";
import fs from "fs";
import path from "path";
import { fileURLToPath, pathToFileURL } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(__dirname, "..");
const out = path.join(root, "tmp-test-skeleton36.cjs");
const assetDir = path.join(root, "public/assets/pet/chaijun");

await build({
  entryPoints: [path.join(root, "src/pet/skeletonBinary36.ts")],
  bundle: true,
  platform: "node",
  format: "cjs",
  outfile: out,
  logLevel: "silent",
});

const { SkeletonBinary36, AtlasAttachmentLoader } = await import(
  pathToFileURL(out).href
);
const { TextureAtlas } = await import("@pixi-spine/base");

const skel = fs.readFileSync(path.join(assetDir, "chaijun.skel"));
const atlasText = fs.readFileSync(path.join(assetDir, "chaijun.atlas"), "utf8");

const atlas = await new Promise((resolve, reject) => {
  new TextureAtlas(
    atlasText,
    (_imagePath, loadTexture) => {
      loadTexture({
        valid: true,
        realWidth: 2048,
        realHeight: 256,
        resolution: 1,
        setSize(w, h) {
          this.realWidth = w;
          this.realHeight = h;
        },
      });
    },
    (loaded) => {
      if (loaded) resolve(loaded);
      else reject(new Error("atlas load failed"));
    },
  );
});

const binary = new SkeletonBinary36(new AtlasAttachmentLoader(atlas));
const data = binary.readSkeletonData(new Uint8Array(skel));

console.log("version:", data.version);
console.log("bones:", data.bones.length);
console.log("slots:", data.slots.length);
console.log("skins:", data.skins.length);
console.log(
  "animations:",
  data.animations.length,
  data.animations.map((a) => a.name).slice(0, 8),
);

fs.unlinkSync(out);
