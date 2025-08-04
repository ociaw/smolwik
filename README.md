# smolwik
`smolwik` is a small and lightweight wiki designed for use at home by individuals, families, or other small groups. It can
be used for a wide variety of purposes, such as notetaking, keeping recipes, or household documentation. `smolwik` is
database-less - all articles and account details are stored as simple TOML and CommonMark files on the filesystem. this
ensures that information can always be accessed by a simple text editor, and can easily be exported and backed up
without worry.

## Features

`smolwik` has very flexible access control, ranging from fully anonymous, to a single user / password, to any number of
user accounts. These modes can be changed between at will, with only a restart of the service required to take effect.
Each article can have viewing or editing privileges restricted to certain users, or opened up completely. Separate
permissions also exist for Page Creation, Administration, and Discovery.

Markdown/CommonMark is the only supported format, with several extensions enabled, including strikethrough and tables.

### Non-Features
There is no versioning. Once an article is changed, there is no backup copy kept. It is recommended to take automated
snapshots and backups of the `articles/` directory at regular intervals.

## Running
Download and extract the latest release from [GitHub](https://github.com/ociaw/smolwik/releases).

Ensure `config.toml` and `accounts.toml` are present, along with the directories `templates/`, `articles/`, and
`assets/` - the locations of these directories can be changed in `config.toml`. Then simply run the `smolwik` binary. By
default, `smolwik` listens at `127.0.0.1:8080`, but this can also be changed in the configuration file.

When started, `smolwik` checks both `config.toml` and `accounts.toml` for security. If the secret key in `config.toml` is
empty or too weak (i.e. less than 64 bytes), that key is ignored and a *new*, secure key is generated. This key exists
only in memory and **will not be saved** automatically.

If running in Single User mode, and the `single_password` property in `accounts.toml` is empty or not a valid hash, then
a *new* password is generated and saved to the `single_password` property in `accounts.toml`. This password is also
printed to standard out, allowing you to save and login with the password.

## Configuration
- `address` - specifies the IPv4 or IPv6 address to listen at, along with the port. Defaults to `127.0.0.1:8080`.
- `secret_key` - the key to perform cryptographic operations such as signing cookies. If empty, will be randomly
on each start up.
- `auth_mode` - Can be `Anonymous`, `Single`, or `Multi`.

Authentication Modes
-----
- Multi-User
- Single-User
- Anonymous

The authentication database is stored in the `accounts.toml` file. Each user has a username and a password hash. Optionally, `smolwik`
can be run in single-user mode, where only a password is required. Authentication can also be disabled entirely,
providing anonymous access only. In this case, articles can only be edited or created through the site if anonymous editing
is enabled.

## Requirements
`smolwik` is designed to run on minimal RAM and CPU power\*. 64MiB and at least 1 CPU core is recommended. It has only
been tested on Linux, however it should be relatively portable to other platforms. If you encounter any problems, please
file an issue.

\*Password hashing via `argon2` is likely the most resource intensive task. If logins are too slow, or memory exhaustion
occurs, RAM and CPU usage can be reduced by tweaking the parameters of `argon2` and [rebuilding](#building) the binary.

# Customization
Stylesheets, images, and JavaScript can be added to the `assets/` directory, which is served to any visitor of the site.

Templates use the [Tera](https://github.com/Keats/tera) templating engine to build the HTML of each page. The `Tera`
language is similar to Jinja2, Django, and Twig. Documentation for Tera can be found [here](https://keats.github.io/tera/).

Assets can be changed at runtime without restarting the server. Template changes require a server restart, with the
exception of `error_fallback.tera`. This requires `smolwik` to be rebuilt - it is generally not recommended to change
the fallback template at all.

## Building
To build, ensure the latest version of rust is installed with the nightly toolchain, then run
```shell
cargo build --release
```
Cargo will automatically download and build all necessary dependencies. This will produce an executable binary at
`./target/release/smolwik`.

Ensure you have at least 1 GB of RAM available to build.

## File Structure
Each article consists of a CommonMark / markdown file with embedded metadata. The metadata stored as TOML, containing
the article title, and which users are allowed to edit and view the file. The CommonMark contents are processed to HTML
(via [pulldown-cmark](https://pulldown-cmark.github.io/pulldown-cmark/cheat-sheet.html)) upon each page load.

# License
The `smolwik` software itself is licensed under AGPL 3.0, however, the templates and assets included are licensed under
the less-restrictive BSD 2-Clause License. This allows private customization of how the wiki looks and feels, while
ensuring the core backend software stays Free.

Rough Idea:

# Similar software
If you like the idea of a database-less wiki, but want to look at other options, here are a few alternatives that fill a
similar niche:
- [DokuWiki](https://www.dokuwiki.org/dokuwiki) - Written in PHP
- [deadwiki](https://crates.io/crates/deadwiki) - Written in Rust, but is unmaintained and archived
