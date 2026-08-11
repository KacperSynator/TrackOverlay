import json

with open("default_config.json", "r") as f:
    config = json.load(f)

# Wait, JSON doesn't support comments officially. I need to ask the user if they meant a specific property or just a fake field like "_comment".
# Or maybe they want it added to README.md? No, the comment specifically said "in default_config.json: for convinience add a comment with available style options" on line 61 which is the style line.
# Many JSON parsers in rust can handle comments if they use `json5` or `json_comments` feature, but `serde_json` strictly rejects them. Let's see if we can use a fake key, or if the user is ok with standard json restrictions.
# Oh, standard serde_json will just ignore extra keys if the struct uses `#[serde(default)]`.
