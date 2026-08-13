---
name: file-operations
description: File operations skill. Use when the user asks to create, read, edit, list, or delete files, or to explore the project directory structure.
---

# File Operations

This skill teaches how to operate on local files using the built-in tools.

## When to Use

Load this skill when:
- Creating, reading, editing, or deleting files
- Listing a directory's contents
- Exploring the project structure
- Writing or updating documentation

## Instructions

To operate on files, use the available tools directly:

1. **List a directory**: use `list_files` with the directory path.
   - Relative paths resolve from the project root.
   - Example: `{"path": "."}` lists the project root.

2. **Read a file**: use `read_file` with the file path.
   - Example: `{"path": "README.md"}` reads the README.

3. **Write a file**: use `write_file` with `path` and `content`.
   - Parent directories are created automatically.
   - Example: `{"path": "notes/hello.txt", "content": "Hello world"}`.

4. **Delete a file**: use `delete_file` with the file path.

## Best Practices

- Always list a directory before assuming what files exist.
- Never fabricate file contents — use `read_file` to check.
- When the user asks to "create" a project, write real files and report the paths.
- If the scope is limited to the project directory and the user asks for a path outside it, explain the limitation instead of failing silently.

## Example Workflow

To create a small project scaffold:

1. `list_files` → check the current structure
2. `write_file` for each new file
3. `list_files` again to confirm

Always report what was created/modified with the full paths.
