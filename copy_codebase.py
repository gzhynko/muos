import os

# List of module directories
modules = ["muos-main", "muos-threads", "muos-syscall"]

# Base directory
base_path = "."

for module in modules:
  src_path = os.path.join(base_path, module, "src")
  for root, _, files in os.walk(src_path):
    for file in files:
      file_path = os.path.join(root, file)
      with open(file_path, "r", encoding="utf-8") as f:
        contents = f.read()
        print(f"--- {file_path} ---")
        print(contents)
        print()

