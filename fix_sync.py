import re

with open('src/sync.rs', 'r') as f:
    content = f.read()

# Extract test module
tests_match = re.search(r'#\[cfg\(test\)\]\nmod tests \{.*\}\n?', content, re.DOTALL)
if tests_match:
    tests_block = tests_match.group(0)

    # Remove test module from content
    content = content.replace(tests_block, '')

    # Extract new functions
    functions_match = re.search(r'(fn find_gopro_crossings.*\}\n\nfn match_lap_crossings.*?\n\})', content, re.DOTALL)
    if functions_match:
        functions_block = functions_match.group(1)

        # Remove new functions from content
        content = content.replace(functions_block, '')

        # Append new functions then test module
        content = content.strip() + '\n\n' + functions_block.strip() + '\n\n' + tests_block.strip() + '\n'

        with open('src/sync.rs', 'w') as f:
            f.write(content)
        print("Success")
    else:
        print("Could not find functions block")

        # We can also just move the test block to the very end
        content = content.strip() + '\n\n' + tests_block.strip() + '\n'
        with open('src/sync.rs', 'w') as f:
            f.write(content)
        print("Moved tests to end instead")
else:
    print("Could not find tests block")
