"""
Remove white background rectangles from exercise SVG files.

The SVGs have a structure like:
  <g id="#ffffffff">
    <path fill="#FFFFFF" d="M 0.00 0.00 L W 0.00 L W H L 0.00 H L 0.00 0.00 M ... Z" />
    <path fill="#FFFFFF" d="..." />
    ...
  </g>

The FIRST path in the white group contains a background rectangle as the first
sub-path (M 0 0 L W 0 L W H L 0 H L 0 0), followed by figure white parts.

This script:
1. Finds the first <path> inside the white group
2. Removes ONLY the background rectangle sub-path (M 0 0 L ... L 0 0)
3. Keeps all remaining sub-paths (figure white parts) intact
4. Does NOT change viewBox, width, height, or any other attributes
5. Does NOT crop or resize anything
"""

import os
import re
import sys
import glob


WHITE_COLOR_RE = re.compile(
    r"""(?ix)
    ^\s*(
        \#(?:f{3}|f{6}|f{8})
        |white
        |rgb\(\s*255\s*,\s*255\s*,\s*255\s*\)
        |rgba\(\s*255\s*,\s*255\s*,\s*255\s*,\s*1(?:\.0+)?\s*\)
        |rgb\(\s*100%\s*,\s*100%\s*,\s*100%\s*\)
        |rgba\(\s*100%\s*,\s*100%\s*,\s*100%\s*,\s*1(?:\.0+)?\s*\)
        |rgb\(\s*255\s+255\s+255(?:\s*/\s*1(?:\.0+)?)?\s*\)
        |hsl\(\s*0\s*,\s*0%\s*,\s*100%\s*\)
        |hsla\(\s*0\s*,\s*0%\s*,\s*100%\s*,\s*1(?:\.0+)?\s*\)
    )\s*$
    """
)


def is_white_color(value):
    return WHITE_COLOR_RE.match(value or "") is not None


def scrub_style_block(text):
    def repl(match):
        prop = match.group(1).lower()
        value = match.group(2).strip()
        if is_white_color(value):
            new_value = "none" if prop in ("fill", "stroke") else "transparent"
            return f"{match.group(1)}:{new_value}"
        return match.group(0)

    return re.sub(
        r"(?i)\b(fill|stroke|stop-color|flood-color|lighting-color|color)\s*:\s*([^;]+)",
        repl,
        text,
    )


def scrub_white_colors(content):
    # Replace white colors in attributes
    def attr_repl(match):
        prop = match.group(1)
        quote = match.group(2)
        value = match.group(3).strip()
        if is_white_color(value):
            new_value = "none" if prop.lower() in ("fill", "stroke") else "transparent"
            return f"{prop}={quote}{new_value}{quote}"
        return match.group(0)

    content = re.sub(
        r"""(?ix)
        \b(fill|stroke|stop-color|flood-color|lighting-color|color)
        \s*=\s*(['"])
        ([^'"]+)
        \2
        """,
        attr_repl,
        content,
    )

    # Replace white colors inside style="..."
    def style_attr_repl(match):
        prefix = match.group(1)
        style_text = match.group(2)
        suffix = match.group(3)
        return prefix + scrub_style_block(style_text) + suffix

    content = re.sub(
        r"""(?is)
        (\bstyle\s*=\s*['"])
        (.*?)
        (['"])
        """,
        style_attr_repl,
        content,
    )

    # Replace white colors inside <style> blocks
    def style_block_repl(match):
        start = match.group(1)
        body = match.group(2)
        end = match.group(3)
        return start + scrub_style_block(body) + end

    content = re.sub(
        r"(?is)(<style\b[^>]*>)(.*?)(</style>)",
        style_block_repl,
        content,
    )

    return content


def remove_bg_rect_from_path(d_attr, svg_width, svg_height):
    """
    Remove the background rectangle sub-path from the 'd' attribute.
    The bg rect is: M 0.00 0.00 L <width> 0.00 L <width> <height> L 0.00 <height> L 0.00 0.00
    It appears at the very start of the 'd' attribute.
    """
    # Build a regex that matches the background rectangle at the start
    # The rectangle goes: M 0 0 L W 0 L W H L 0 H L 0 0
    # Values can be integers or floats (e.g. 167.00, 175, etc.)
    # After the rect, there's usually " M " starting the next sub-path
    
    w = float(svg_width)
    h = float(svg_height)
    
    # Pattern: M 0.00 0.00 L <w> 0.00 L <w> <h> L 0.00 <h> L 0.00 0.00
    # Allow flexible number formatting (int or float)
    def num_pat(val):
        """Create a pattern matching a number that equals val."""
        int_val = int(val)
        if val == int_val:
            # Match both "167" and "167.00" etc.
            return rf'{int_val}(?:\.0+)?'
        else:
            return re.escape(f'{val:.2f}')
    
    zero = r'0(?:\.0+)?'
    wp = num_pat(w)
    hp = num_pat(h)
    
    # The background rect pattern
    bg_pattern = (
        rf'^\s*M\s+{zero}\s+{zero}\s+'
        rf'L\s+{wp}\s+{zero}\s+'
        rf'L\s+{wp}\s+{hp}\s+'
        rf'L\s+{zero}\s+{hp}\s+'
        rf'L\s+{zero}\s+{zero}\s*'
    )
    
    match = re.match(bg_pattern, d_attr)
    if match:
        remaining = d_attr[match.end():].strip()
        if remaining:
            return remaining
        else:
            # The entire path was just the background rect - return empty
            return None
    
    return d_attr  # No change if pattern not found


def process_svg_file(filepath):
    """Process a single SVG file to remove its white background rectangle and white colors."""
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Extract width and height from the svg element's viewBox
    viewbox_match = re.search(r'viewBox="(\d+(?:\.\d+)?)\s+(\d+(?:\.\d+)?)\s+(\d+(?:\.\d+)?)\s+(\d+(?:\.\d+)?)"', content)
    if not viewbox_match:
        print(f"  SKIP (no viewBox): {filepath}")
        return False
    
    vb_w = float(viewbox_match.group(3))
    vb_h = float(viewbox_match.group(4))
    
    changed = False

    # Remove entire white group(s) to avoid CSS re-coloring white shapes.
    # The white group is typically id="#ffffffff" (sometimes #fff/#ffffff variants).
    new_content = re.sub(
        r'(?is)\s*<g\b[^>]*\bid\s*=\s*["\']#f{3,8}["\'][^>]*>.*?</g>\s*',
        '',
        content,
    )
    if new_content != content:
        content = new_content
        changed = True

    # As a fallback, remove the background rectangle sub-path from any path that
    # starts with the full viewBox rect (regardless of fill color).
    def strip_bg_rect_from_paths(text):
        def repl(match):
            tag = match.group(0)
            d_match = re.search(r'(?is)\bd\s*=\s*([\'"])(.*?)\1', tag)
            if not d_match:
                return tag
            d_attr = d_match.group(2)
            new_d = remove_bg_rect_from_path(d_attr, vb_w, vb_h)
            if new_d is None:
                return ''
            if new_d == d_attr:
                return tag
            return tag[:d_match.start(2)] + new_d + tag[d_match.end(2):]

        return re.sub(r'(?is)<path\b[^>]*>', repl, text)

    stripped = strip_bg_rect_from_paths(content)
    if stripped != content:
        content = stripped
        changed = True

    # Remove any remaining white fills/strokes/colors globally
    new_content = scrub_white_colors(content)
    if new_content != content:
        content = new_content
        changed = True

    if changed:
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(content)

    return changed


def main():
    data_dir = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'exercises', 'data')
    
    if not os.path.isdir(data_dir):
        print(f"Data directory not found: {data_dir}")
        sys.exit(1)
    
    svg_files = glob.glob(os.path.join(data_dir, '**', '*.svg'), recursive=True)
    svg_files.sort()
    
    print(f"Found {len(svg_files)} SVG files")
    
    processed = 0
    skipped = 0
    errors = 0
    
    for svg_file in svg_files:
        rel = os.path.relpath(svg_file, data_dir)
        try:
            if process_svg_file(svg_file):
                processed += 1
                print(f"  OK: {rel}")
            else:
                skipped += 1
        except Exception as e:
            errors += 1
            print(f"  ERROR: {rel} - {e}")
    
    print(f"\nDone! Processed: {processed}, Skipped: {skipped}, Errors: {errors}")


if __name__ == '__main__':
    main()
