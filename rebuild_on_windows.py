import os
import subprocess

root_path = os.path.dirname(__file__)

subprocess.run (["Taskkill", "/IM", "borg_the_spire_copy_2.exe", "/F"])
subprocess.run (["cargo", "build"], cwd=root_path)
