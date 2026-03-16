# Strex Tutorials

Step-by-step collections that walk you through every feature in order.
Each file is self-contained and runnable with a single command.

All tutorials use [JSONPlaceholder](https://jsonplaceholder.typicode.com) (GET/HEAD)
and [httpbin.org](https://httpbin.org) (POST bodies) — both are free and require no signup.

---

## 01 · Hello World

```
strex run examples/tutorials/01-hello-world.yaml
```

One GET request, one status assertion.  
Learn the minimum required fields: `name`, `version`, `requests`, `method`, `url`, `assertions`.

---

## 02 · Assertions

```
strex run examples/tutorials/02-assertions.yaml
```

Every declarative assertion type:

| Kind | Options |
|------|---------|
| `status` | exact HTTP status code |
| `jsonPath` | `exists`, `equals`, `contains` |
| `header` | `exists`, `equals`, `contains` |

---

## 03 · Variables and Environment

```
strex run examples/tutorials/03-variables-and-environment.yaml
```

- `environment` — static config, available as `{{key}}` everywhere
- `variables` — mutable runtime state, written by scripts
- `{{name}}` interpolation syntax and resolution order

---

## 04 · Request Chaining

```
strex run examples/tutorials/04-request-chaining.yaml
```

- `pre_script` — runs before the request; can set variables used in headers/body/URL
- `post_script` — runs after the response; extracts data for subsequent requests
- Full create → read → update → delete lifecycle

---

## 05 · Scripting

```
strex run examples/tutorials/05-scripting.yaml
```

Complete JavaScript API reference with working examples:

| Object | Key methods / properties |
|--------|--------------------------|
| `response` | `.status`, `.statusText`, `.headers`, `.text()`, `.json()`, `.timing.total` |
| `variables` | `.get()`, `.set()`, `.has()`, `.keys()`, `.delete()`, `.clear()` |
| `env` | `.get()` |
| assertions | `assert()`, `assertEqual()`, `assertNotEqual()`, `assertContains()`, `assertMatch()` |
| `console` | `.log()`, `.warn()`, `.error()` |

---

## 06 · Data-Driven Testing

```
# CSV
strex run examples/tutorials/06-data-driven.yaml \
          --data examples/tutorials/06-users.csv

# JSON
strex run examples/tutorials/06-data-driven.yaml \
          --data examples/tutorials/06-users.json

# Concurrent (3 rows at a time)
strex run examples/tutorials/06-data-driven.yaml \
          --data examples/tutorials/06-users.csv \
          --concurrency 3
```

- `{{columnName}}` — resolved from the current data row
- Variable isolation — each row gets a fresh scope; scripts cannot leak across rows
- `--concurrency N` — run N rows in parallel (default: 1)
- `--fail-fast` — stop on first failing row

Data files included: `06-users.csv`, `06-users.json`

---

## 07 · on_failure Control Flow

```
strex run examples/tutorials/07-on-failure.yaml
```

Control what happens to the rest of the collection when a request fails:

| Value | Behaviour |
|-------|-----------|
| *(omitted)* | Log failure, continue to next request (default) |
| `on_failure: stop` | Abort — all remaining requests are skipped |
| `on_failure: {skip_to: "Name"}` | Skip requests until the named step, then resume |

Triggered by: any assertion failure OR a script that throws.  
Skipped requests appear as `-` in the console, `"skipped": true` in JSON output,
and `<skipped/>` in JUnit XML.
