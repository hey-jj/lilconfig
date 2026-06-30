# lilconfig

Find and load a tool's config file by searching up the directory tree.

Give it a tool name. It derives a set of conventional filenames, walks up from a
starting directory, and returns the first file that exists and parses. The
search and selection rules match cosmiconfig.

## Installation

```toml
[dependencies]
lilconfig = "0.1"
```

## Usage

Search upward from the current directory:

```rust
use lilconfig::SearcherBuilder;

let searcher = SearcherBuilder::new("myapp").build()?;
if let Some(found) = searcher.search_cwd()? {
    println!("config at {}: {:?}", found.filepath.display(), found.config);
}
```

Load one file by path:

```rust
use lilconfig::SearcherBuilder;

let searcher = SearcherBuilder::new("myapp").build()?;
let result = searcher.load("myapp.config.json")?;
```

An async surface mirrors the sync one. Build it with `AsyncSearcherBuilder::new`
and await `search` and `load`.

## Default search places

For a tool named `myapp`, search tries these names in each directory, in order:

```
package.json
.myapprc
.myapprc.json
.config/myapprc
.config/myapprc.json
```

`package.json` is special: the loader reads the key named after the tool (or the
key set with `package_prop`) and treats a missing key as no match.

## Loaders

A loader turns file text into a `serde_json::Value`. The defaults parse JSON for
`.json` files and for extensionless files. Register more with `loader`:

```rust
use lilconfig::{loader, SearcherBuilder};
use serde_json::Value;

let searcher = SearcherBuilder::new("myapp")
    .loader(".toml", loader(|_path, text| {
        // parse `text` into a Value
        Ok(Value::Null)
    }))
    .build()?;
```

JavaScript config files (`.js`, `.cjs`, `.mjs`) have no default loader because
this crate does not execute code. Register your own if you need them.

## Config values

`SearchResult.config` is `Option<Value>`. `None` means the file was empty.
`Some(Value::Null)` is an explicit null config. A search that finds nothing
returns `None` for the whole result.

## License

Licensed under the [MIT license](LICENSE).
