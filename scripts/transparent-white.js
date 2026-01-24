import path from "node:path";
import { Jimp } from "jimp";

const defaultInput = path.resolve(
  "tauri/icons/tray/macos/statusbar_1024.png"
);

const inputPath = process.argv[2]
  ? path.resolve(process.argv[2])
  : defaultInput;
const outputPath = process.argv[3]
  ? path.resolve(process.argv[3])
  : inputPath;
const threshold = process.argv[4] ? Number(process.argv[4]) : 245;

const image = await Jimp.read(inputPath);

image.scan(0, 0, image.bitmap.width, image.bitmap.height, function (x, y, idx) {
  const { data } = this.bitmap;
  const r = data[idx + 0];
  const g = data[idx + 1];
  const b = data[idx + 2];
  const a = data[idx + 3];

  if (a !== 0 && r >= threshold && g >= threshold && b >= threshold) {
    data[idx + 3] = 0;
  }
});

await image.write(outputPath);

console.log(`Done: ${outputPath}`);
