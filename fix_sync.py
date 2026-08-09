import re

with open('src/sync.rs', 'r') as f:
    content = f.read()

# I see the problem. My regex replace deleted the FIRST test block but then appended it again.
# Let's cleanly separate the file content before test module, the functions, and the test module.

test_idx = content.find('#[cfg(test)]\nmod tests {')
if test_idx != -1:
    before_tests = content[:test_idx]
    after_tests = content[test_idx:]

    # In `after_tests`, we have the `tests` mod and the `find_gopro_crossings` and `match_lap_crossings` functions.
    # Let's extract the functions from `after_tests`.
    func1_idx = after_tests.find('fn find_gopro_crossings')

    if func1_idx != -1:
        test_module = after_tests[:func1_idx]
        funcs = after_tests[func1_idx:]

        final_content = before_tests + funcs + '\n\n' + test_module
        with open('src/sync.rs', 'w') as f:
            f.write(final_content)
        print("Fixed correctly.")
    else:
        print("funcs not found")
