import re
import glob
import os

def process_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    # Match numeric constants that are multiplied by el.scale or multiplied/divided by res_scale
    # Pattern: a float followed by " * el.scale" or " * res_scale"
    # Example: 32.0 * el.scale

    # We want to replace matching constants with their value * 1.5
    def repl(m):
        val = float(m.group(1))
        new_val = val * 1.5
        # return formatted string to preserve .0 if it was there
        if new_val.is_integer():
            return f"{new_val:.1f}{m.group(2)}"
        return f"{new_val}{m.group(2)}"

    # Regex to find: (NUMBER) (* el.scale)
    # We also have cases like 24.0 * ctx.el.scale
    # Let's target numbers multiplied by el.scale or ctx.el.scale
    new_content = re.sub(r'(\d+\.\d+)(\s*\*\s*(?:ctx\.)?el\.scale)', repl, content)

    # We might have something multiplied by just res_scale, but usually it's `val * el.scale * res_scale`
    # and the above regex will catch the `val * el.scale` part.

    # Check for cases where it's DEFAULT_LED_RADIUS (which is 15.0) -> that's in common.rs or something

    with open(filepath, 'w') as f:
        f.write(new_content)

for filepath in glob.glob('src/overlay/**/*.rs', recursive=True):
    process_file(filepath)
