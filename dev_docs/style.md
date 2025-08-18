# Project Style

This project is extensively developed with llms, and requires affordances for humans and llms to be explicitly written into the repository. This means context is written into .md files for long term knowledge across multiple llm chat sessions.

This project uses Makefiles as a task runner for building, testing, and formatting and keeping other functionality in one place.

The folder organization prefers flatness over deep nesting. 

You are often going to be generating code that is not complete. Leave TODOs and reference the DTKT that will complete the task from project_plan.md. Create new DTKT task if necessary.

When adding new dependencies, show or run the command for the package manger to install the dependency rather than modifying the dependencies definitions directly.

Implementation for individual cli subcommands should be in the src/commands/ subfolder.

Unit tests should go in the same file as the functionality they are testing.

Comments should explain why something is done, not what is done. Prefer no or few comments over a lot of comments.

When working on feature, do a git diff against main, and make sure that no unnecessary or temporary code snuck in.

