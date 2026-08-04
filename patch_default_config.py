import json

with open('default_config.json', 'r') as f:
    config = json.load(f)

# Put LapTimer back, disabled
for el in config.get('elements', []):
    if el.get('kind') == 'AdvancedLapTimer':
        el['kind'] = 'LapTimer'
        el['enabled'] = False

# Add AdvancedLapTimer
config['elements'].insert(2, {
  "enabled": True,
  "kind": "AdvancedLapTimer",
  "x": 0.1,
  "y": 0.1,
  "scale": 1.0
})

with open('default_config.json', 'w') as f:
    json.dump(config, f, indent=2)
