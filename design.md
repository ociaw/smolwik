Rough Idea:

Simple wiki-like site with pages that can be edited in the browser. `smolwik` is a very lightweight, personal wiki site
to keep personal notes and home-networking documentation. The page contents are saved as simple text files, allowing
easy backups, export, or even viewing/editing outside of the wiki software. Users and authentication are supported,
and viewing and editing of pages can be restricted on a page-by-page basis.

Each page consists of a markdown file, and a metadata file. The markdown file contains the pages contents, and is
processed to HTML via a markdown parser/formatter. The metadata determines things like the page title, and which users
are allowed to edit and view the file.

Performance
-----------
`smolwik` aims to use the minimum of CPU and memory usage, so that it can be run in memory-starved containers.

Authentication Modes
-----
- Multi-User
- Single-User
- Anonymous

The authentication database is stored in a SQLite file. Each user has a username and a password. Optionally, `smolwik` 
can be run in single-user mode, where only a password is required. Authentication can also be disabled entirely,
providing anonymous access only. In this case, pages can only be edited or created through the site if anonymous editing
is enabled.

Styling
-------
A custom CSS file can be loaded. Further changes to the HTML structure of the site will require modification of the
source code and a new binary to be built.
