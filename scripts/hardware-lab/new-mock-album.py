"""Builds a mock album for the hardware lab: camera-sized JPEGs (24-30 MP,
~7-13 MB each, with camera EXIF and some portraits stored through the EXIF
orientation) in three folders, and a .myalbuns project whose Sheets already
hold Photos. The pixels mix a smooth color field, mid-frequency texture and
grain so the files compress like real photos instead of a few kilobytes.

Needs Pillow and numpy. Usage:
    python scripts/hardware-lab/new-mock-album.py --output D:/HardwareLab/fixtures/album-mock
"""

import argparse
import json
import random
import uuid
from datetime import datetime, timedelta
from multiprocessing import Pool
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

FOLDERS = [("Making of", 0.2), ("Cerimônia", 0.4), ("Festa", 0.4)]
SHEET_WIDTH_MM, SHEET_HEIGHT_MM = 600, 300

# Frame templates of a double Sheet in millimetres: (x, y, width, height).
TEMPLATES = [
    [(15, 52.5, 270, 180), (315, 52.5, 270, 180)],
    [(60, 15, 180, 270), (315, 15, 270, 132.5), (315, 152.5, 270, 132.5)],
    [(15, 15, 132.5, 132.5), (152.5, 15, 132.5, 132.5), (15, 152.5, 132.5, 132.5),
     (152.5, 152.5, 132.5, 132.5), (315, 15, 270, 270)],
    [(15, 15, 270, 270), (315, 15, 132.5, 270), (452.5, 15, 132.5, 270)],
    [(15, 15, 570, 270)],
]


def render_photo(task):
    index, path, label, width, height, orientation, grain, texture, taken = task
    rng = np.random.default_rng(index)
    base = Image.fromarray(rng.integers(30, 230, (6, 9, 3), dtype=np.uint8)).resize(
        (width, height), Image.BICUBIC)
    mid = Image.fromarray(rng.integers(0, 255, (height // 24, width // 24, 3), dtype=np.uint8)).resize(
        (width, height), Image.BICUBIC)
    pixels = np.asarray(base, dtype=np.int16)
    pixels += ((np.asarray(mid, dtype=np.int16) - 128) * texture).astype(np.int16)
    pixels += rng.normal(0, grain, (height, width, 1)).astype(np.int16)
    image = Image.fromarray(np.clip(pixels, 0, 255).astype(np.uint8))

    draw = ImageDraw.Draw(image)
    font = ImageFont.truetype("arial.ttf", min(width, height) // 9)
    draw.text((width // 2, height // 2), label, font=font, anchor="mm", fill="white",
              stroke_width=max(4, min(width, height) // 200), stroke_fill="black")
    if orientation == 6:
        # Stored rotated: viewers turn it 90 degrees clockwise back upright.
        image = image.transpose(Image.ROTATE_90)

    exif = Image.Exif()
    stamp = taken.strftime("%Y:%m:%d %H:%M:%S")
    exif[0x010F] = "Canon"
    exif[0x0110] = "Canon EOS R6"
    exif[0x0112] = orientation
    exif[0x0132] = stamp
    exif.get_ifd(0x8769)[0x9003] = stamp
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path, "JPEG", quality=92, subsampling=1, exif=exif)
    return path.stat().st_size


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True)
    parser.add_argument("--photos", type=int, default=300)
    parser.add_argument("--sheets", type=int, default=40)
    parser.add_argument("--workers", type=int, default=3)
    parser.add_argument("--seed", type=int, default=1)
    arguments = parser.parse_args()

    output = Path(arguments.output).resolve()
    pick = random.Random(arguments.seed)
    new_id = lambda: str(uuid.UUID(int=pick.getrandbits(128), version=4))
    start = datetime(2026, 5, 16, 14, 0, 0)

    photos = []
    for folder, share in FOLDERS:
        for _ in range(round(arguments.photos * share)):
            number = len(photos) + 1
            portrait = pick.random() < 0.3
            width, height = (6720, 4480) if pick.random() < 0.15 else (6000, 4000)
            orientation = 1
            if portrait:
                width, height = height, width
                # Most portraits arrive as landscape pixels plus an EXIF turn.
                orientation = 6 if pick.random() < 0.6 else 1
            photos.append({
                "id": new_id(),
                "folder": folder,
                "path": output / "Fotos" / folder / f"IMG_{number:04d}.JPG",
                "portrait": portrait,
                "task": (arguments.seed * 100_000 + number, output / "Fotos" / folder / f"IMG_{number:04d}.JPG",
                         f"{folder} {number:03d}",
                         width, height, orientation, pick.uniform(6, 10), pick.uniform(0.28, 0.4),
                         start + timedelta(seconds=37 * number)),
            })

    tasks = [photo["task"] for photo in photos if not photo["path"].exists()]
    print(f"{len(photos)} photos, {len(tasks)} to render with {arguments.workers} workers")
    with Pool(arguments.workers) as pool:
        for done, _ in enumerate(pool.imap_unordered(render_photo, tasks, chunksize=2), start=1):
            if done % 20 == 0 or done == len(tasks):
                print(f"  {done}/{len(tasks)}", flush=True)

    # Fill each Sheet slot with the next unused Photo of the same orientation.
    queues = {True: [p for p in photos if p["portrait"]], False: [p for p in photos if not p["portrait"]]}
    um = lambda millimetres: round(millimetres * 1000)
    sheets = []
    for sheet_index in range(arguments.sheets):
        frames = []
        for x, y, width, height in TEMPLATES[sheet_index % len(TEMPLATES)]:
            frame = {"id": new_id(), "xUm": um(x), "yUm": um(y), "widthUm": um(width), "heightUm": um(height)}
            queue = queues[height > width] or queues[height <= width]
            if queue:
                frame["photo"] = {"mediaId": queue.pop(0)["id"],
                                  "transform": {"panX": 0, "panY": 0, "userZoom": 1}}
            frames.append(frame)
        sheets.append({"id": new_id(), "activeSides": "both", "frames": frames})

    project = {
        "documentType": "myalbuns.project",
        "schemaVersion": 1,
        "projectId": new_id(),
        "revision": 1,
        "project": {
            "album": {"displayUnit": "mm", "sheetWidthUm": um(SHEET_WIDTH_MM), "sheetHeightUm": um(SHEET_HEIGHT_MM),
                      "dpi": 300, "bleedUm": 3000, "safetyUm": 3000},
            "layoutSettings": {"permission": "pagesAndSheet", "marginUm": 15000, "gapUm": 5000,
                               "minimumSideUm": 20000},
            "visualDefaults": {"background": {"sides": "both", "both": {"kind": "color", "rgb": "#FFFFFF"}},
                               "overlay": {"sides": "both", "both": {"kind": "none"}},
                               "frameBorder": {"kind": "none"}},
            "media": [{"id": p["id"], "kind": "photo", "path": str(p["path"])} for p in photos],
            "mediaFolders": [{"id": new_id(), "kind": "photo", "name": folder,
                              "mediaIds": [p["id"] for p in photos if p["folder"] == folder]}
                             for folder, _ in FOLDERS],
            "sheets": sheets,
        },
    }
    project_path = output / "album-mock.myalbuns"
    project_path.write_text(json.dumps(project, ensure_ascii=False, indent=2), encoding="utf-8")
    placed = sum(1 for sheet in sheets for frame in sheet["frames"] if "photo" in frame)
    total = sum(p["path"].stat().st_size for p in photos)
    print(f"{project_path}: {len(sheets)} sheets, {placed} placed photos, {total / 2**30:.2f} GB of photos")


if __name__ == "__main__":
    main()
