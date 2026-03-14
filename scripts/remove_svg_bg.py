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
    """Process a single SVG file to remove its white background rectangle."""
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Extract width and height from the svg element's viewBox
    viewbox_match = re.search(r'viewBox="(\d+(?:\.\d+)?)\s+(\d+(?:\.\d+)?)\s+(\d+(?:\.\d+)?)\s+(\d+(?:\.\d+)?)"', content)
    if not viewbox_match:
        print(f"  SKIP (no viewBox): {filepath}")
        return False
    
    vb_w = float(viewbox_match.group(3))
    vb_h = float(viewbox_match.group(4))
    
    # Find the white group and its first path
    # Pattern: <g id="#ffffffff"> ... first <path ...> 
    white_group_match = re.search(
        r'(<g\s+id="#fff(?:fffff)?f?">\s*\n\s*<path\s+fill="#(?:FFFFFF|ffffff)"\s+opacity="1\.00"\s+d=")(.*?)("\s*/>)',
        content, re.DOTALL
    )
    
    if not white_group_match:
        print(f"  SKIP (no white group/path found): {filepath}")
        return False
    
    prefix = white_group_match.group(1)
    d_attr = white_group_match.group(2)
    suffix = white_group_match.group(3)
    
    new_d = remove_bg_rect_from_path(d_attr, vb_w, vb_h)
    
    if new_d is None:
        # Entire path was just background - remove the whole path element
        new_content = content[:white_group_match.start()] + \
            re.sub(
                r'<g\s+id="#fff(?:fffff)?f?">\s*\n\s*<path\s+fill="#(?:FFFFFF|ffffff)"\s+opacity="1\.00"\s+d=".*?"\s*/>',
                lambda m: m.group(0).split('\n')[0] + '\n',  # Keep group, remove path
                content[white_group_match.start():white_group_match.end()],
                count=1, flags=re.DOTALL
            ) + content[white_group_match.end():]
        # Actually, just remove the path line
        full_match = white_group_match.group(0)
        # Replace just the path with nothing, keeping the <g> tag
        g_tag_match = re.match(r'(<g\s+id="#fff(?:fffff)?f?">\s*\n\s*)', prefix)
        if g_tag_match:
            new_content = content[:white_group_match.start()] + g_tag_match.group(1) + content[white_group_match.end():]
        else:
            new_content = content
    elif new_d == d_attr:
        print(f"  SKIP (no bg rect found in path): {filepath}")
        return False
    else:
        new_content = content[:white_group_match.start()] + prefix + new_d + suffix + content[white_group_match.end():]
    
    with open(filepath, 'w', encoding='utf-8') as f:
        f.write(new_content)
    
    return True


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
