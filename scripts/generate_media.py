import json
import glob
import os
from PIL import Image, ImageDraw, ImageFont

FONT_PATH = "/usr/share/fonts/Adwaita/AdwaitaMono-Regular.ttf"
if not os.path.exists(FONT_PATH):
    FONT_PATH = "/usr/share/fonts/liberation/LiberationMono-Bold.ttf"

FONT_SIZE = 15
CHAR_WIDTH = 9
CHAR_HEIGHT = 19
PADDING = 24
HEADER_HEIGHT = 38

font = ImageFont.truetype(FONT_PATH, FONT_SIZE)

def render_frame_to_image(json_path):
    with open(json_path, "r") as f:
        data = json.load(f)

    cols = data["width"]
    rows = data["height"]

    term_width = cols * CHAR_WIDTH
    term_height = rows * CHAR_HEIGHT

    img_width = term_width + PADDING * 2
    img_height = term_height + HEADER_HEIGHT + PADDING * 2

    # Modern developer terminal dark background
    img = Image.new("RGBA", (img_width, img_height), (9, 12, 17, 255))
    draw = ImageDraw.Draw(img)

    # Window body
    win_x0 = PADDING
    win_y0 = PADDING
    win_x1 = img_width - PADDING
    win_y1 = img_height - PADDING

    # Rounded terminal window background
    draw.rounded_rectangle([win_x0, win_y0, win_x1, win_y1], radius=10, fill=(13, 17, 24, 255), outline=(45, 58, 76, 255), width=1)

    # Titlebar
    draw.rounded_rectangle([win_x0, win_y0, win_x1, win_y0 + HEADER_HEIGHT], radius=10, fill=(20, 26, 36, 255))
    draw.rectangle([win_x0, win_y0 + HEADER_HEIGHT - 6, win_x1, win_y0 + HEADER_HEIGHT], fill=(20, 26, 36, 255))
    draw.line([win_x0, win_y0 + HEADER_HEIGHT, win_x1, win_y0 + HEADER_HEIGHT], fill=(35, 45, 60, 255), width=1)

    # Window control dots
    dots = [
        (win_x0 + 16, win_y0 + 19, (255, 95, 86)),
        (win_x0 + 34, win_y0 + 19, (255, 189, 46)),
        (win_x0 + 52, win_y0 + 19, (39, 201, 63)),
    ]
    for x, y, col in dots:
        draw.ellipse([x - 5, y - 5, x + 5, y + 5], fill=col)

    # Window title
    title = "prefixpug (ratatui)"
    title_w = draw.textlength(title, font=font)
    draw.text((win_x0 + (term_width - title_w) // 2, win_y0 + 10), title, font=font, fill=(160, 185, 210))

    # Render terminal cells
    grid_x0 = win_x0
    grid_y0 = win_y0 + HEADER_HEIGHT

    for cell in data["cells"]:
        x = cell["x"]
        y = cell["y"]
        ch = cell["ch"]
        fg = tuple(cell["fg"])
        bg = tuple(cell["bg"])

        px = grid_x0 + x * CHAR_WIDTH
        py = grid_y0 + y * CHAR_HEIGHT

        if bg != (13, 17, 24):
            draw.rectangle([px, py, px + CHAR_WIDTH, py + CHAR_HEIGHT], fill=bg)

        if ch and ch != " ":
            draw.text((px, py), ch, font=font, fill=fg)

    return img

def main():
    frame_files = sorted(glob.glob("/tmp/pug_render_frames/frame_*.json"))
    if not frame_files:
        print("No frames found!")
        return

    os.makedirs("/home/papab/Projects/prefixpug/assets", exist_ok=True)

    print("Generating static screenshot assets/prefixpug_tui.png...")
    img_main = render_frame_to_image(frame_files[3])
    img_main.save("/home/papab/Projects/prefixpug/assets/prefixpug_tui.png")

    print("Generating modal screenshot assets/prefixpug_modal.png...")
    # Find frame with modal
    img_modal = render_frame_to_image(frame_files[24])
    img_modal.save("/home/papab/Projects/prefixpug/assets/prefixpug_modal.png")

    print(f"Generating animated GIF from {len(frame_files)} frames...")
    images = [render_frame_to_image(f) for f in frame_files]

    # Repeat some frames for pause at start and modal
    gif_frames = []
    for i, frame in enumerate(images):
        dur = 120
        if i == 0 or i == 24: # pause on initial and confirmation
            for _ in range(4):
                gif_frames.append(frame.convert("RGB"))
        else:
            gif_frames.append(frame.convert("RGB"))

    gif_frames[0].save(
        "/home/papab/Projects/prefixpug/assets/prefixpug_demo.gif",
        save_all=True,
        append_images=gif_frames[1:],
        duration=120,
        loop=0,
        optimize=True
    )
    gif_frames[0].save(
        "/home/papab/Projects/prefixpug/assets/prefixpug_tui_demo.gif",
        save_all=True,
        append_images=gif_frames[1:],
        duration=120,
        loop=0,
        optimize=True
    )
    print("✓ Successfully generated:")
    print("  - assets/prefixpug_tui.png")
    print("  - assets/prefixpug_modal.png")
    print("  - assets/prefixpug_demo.gif")
    print("  - assets/prefixpug_tui_demo.gif")

if __name__ == "__main__":
    main()
