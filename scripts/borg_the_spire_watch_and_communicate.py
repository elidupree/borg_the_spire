# It would be nice if everything could be implemented within the Rust app itself.
# Unfortunately, there's a bit of a bootstrapping issue:
# we want to auto-rerun the executable in /target/ when it's modified,
# but at least on Windows, you're not allowed to modify the executable while it's being run!
# So we have this little script to patch it together.

import os
import subprocess
import shutil

root_path = os.path.dirname(os.path.dirname(__file__))
executable_original = os.path.join(root_path, "target/debug/borg_the_spire.exe")
executable_copy_1 = os.path.join(root_path, "target/borg_the_spire/borg_the_spire_copy_1.exe")
executable_copy_2 = os.path.join(root_path, "target/borg_the_spire/borg_the_spire_copy_2.exe")
last_state_path = os.path.join(root_path, "data/last_state.json")
os.makedirs(os.path.join(root_path, "target/borg_the_spire"), exist_ok = True)
shutil.copyfile(executable_original, executable_copy_1)

subprocess.run ([executable_copy_1, "watch", executable_original, executable_copy_2, "communicate", last_state_path])
