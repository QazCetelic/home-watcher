# `home-watcher`

`home-watcher` is a Linux utility that monitors your home directory for file creation activities outside of the [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/latest/) and logs these interactions into an SQLite database.
It's usefull for finding out which applications keep creating random files with such incredibly descriptive names such as `-6000.`, `.swo`, and `.serverauth.11972` 🤔.

## Usage

Home Watcher will need to run as root to access the Linux Audit system, but does not need any arguments.
```sh
sudo home-watcher
```

### Options
| Option                | Description                                                                                  | Default                         |
|-----------------------|----------------------------------------------------------------------------------------------|---------------------------------|
| `-e, --excluded-dirs` | Specify additional directories to exclude from monitoring (relative to your home directory). | None                            |
| `-l, --log-file`      | Path to the SQLite database file for logging.                                                | `XDG_DATA_HOME/home-watcher.db` |
| `-i, --interval`      | Interval (in milliseconds) to check audit logs.                                              | 1500                            |
| `-u, --user`          | The user to watch the home directory of.                                                     | Executing user                  |
| `-h, --help`          | Display the help message with usage details.                                                 |                                 |
| `-V, --version`       | Display the current version of `home-watcher`.                                               |                                 |

Note: Common user directories like `Documents`, `Downloads`, `Pictures`, `Videos`, `Desktop`, and `Music` are excluded by default.

## Installation

1. **Build from Source**  
   Clone the repository and compile the Rust project:
   ```bash
   git clone https://github.com/QazCetelic/home-watcher.git
   cd home-watcher
   cargo build --release
   ```
   The compiled binary will be available in the `target/release` directory.

2. **Install the Binary**  
   Move the binary to a directory in your `PATH`, such as `/usr/local/bin`:
   ```bash
   sudo mv target/release/home-watcher /usr/local/bin/
   ```

## Database
The database contains an `interactions` table with the following data.
| **Name**   | **Column** | **Description**                                          |
|------------|------------|----------------------------------------------------------|
| **Id**     | `id`       | Identifies a specific interaction                        |
| **Time**   | multiple   | When the file or directory was created                   |
| **File**   | `file`     | The location of the newly created file or directory      |
| **Source** | `source`   | The path of the program responsible for the creation     |

## Contributing

If you find bugs, have suggestions, or want to contribute features, feel free to open an issue or submit a pull request.
