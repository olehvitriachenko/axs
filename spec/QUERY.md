# AXS Queries

**Status:** Experimental, implemented by the Rust crate (`axs::query`, `axs query`)

A query selects, filters, and projects part of a document so that only the
data a model needs reaches its context. Queries work on the data model, so they
apply equally to AXS documents and JSON converted to AXS.

```text
users                         the whole array
users[0]   users[-1]          first and last element
users[age > 20]               elements matching a condition
users[age > 20]{name,email}   ... keeping only some fields
users.name                    one field of every element
issues[state == open and labels.name ~ bug]{number,title}
```

## Steps

A query starts at the document root and applies steps from left to right. The
first step may be a field name without a leading `.`; `.` alone or an empty
query is the whole document.

| Step | On an object | On an array |
|---|---|---|
| `.name` | that field; an error if missing | `name` of every object element, skipping elements without it; nested arrays are mapped, keeping their shape |
| `{a,b}` | an object with only those fields, in that order; missing fields are left out | the same for every object element |
| `[N]` | error | the element at `N`; negative counts from the end (`-1` is the last); an error if out of range |
| `[*]` | the values, as an array | the array itself |
| `[condition]` | error | the elements for which the condition holds |

Field names follow AXS bare keys without `.`. Any other name is quoted:
`"@id"`, `"a.b"`, `"first name"`. Spaces are allowed between steps and inside
brackets.

## Conditions

A condition compares a field of the element with a literal:

```text
field op literal
```

- `field` is a path relative to the element (`age`, `user.login`), or `@` for the
  element itself (for arrays of primitives: `tags[@ ~ rust]`). Through an array,
  a path collects that field of every object element, so `labels.name` is the
  list of label names. If the path does not exist, the condition is false.
- `op` is one of `==`, `!=`, `>`, `>=`, `<`, `<=`, `~`.
- `literal` is read like an AXS scalar: `22`, `1.5`, `true`, `null`, a bare
  word such as `open` (a string), or a quoted string such as `"in progress"`.
  A quoted number is a string: `age == "22"` never matches the integer 22.

Combine conditions with `and` and `or`; `and` binds tighter. Use parentheses to
group: `(city == Lviv or city == Kyiv) and age > 21`.

Comparison rules:

- `==` and `!=`: integers and floats compare numerically (`22 == 22.0`); other
  values must be equal in type and value.
- `<`, `<=`, `>`, `>=`: numbers compare numerically and strings by Unicode
  code point order. Any other combination is false, not an error.
- `~` (contains): a string contains the literal string as a substring
  (case-sensitive), or an array contains an element equal to the literal.

There are no functions, variables, aggregations, or sorting. They may be added
later without changing the meaning of existing queries.

## Results and errors

The result is any AXS value. `axs query` writes an object result as a
document and any other result under a `result` key, so arrays of uniform
objects come out as tables:

```console
$ axs query --from-json -- 'items[state == open]{number,title,comments}' issues.json
result[21]{number,title,comments}:
  1001,Query filter response error fix update,10
  1002,Array token table cache number,8
  ...
```

On the benchmark's GitHub issues dataset that result is 310 tokens, against
3,937 for the full document as minified JSON.

Errors use code `AXS012` with a span in the query (line 1, Unicode scalar
columns): syntax errors, a missing field (with the available field names as
help), an index out of range, or a step applied to the wrong type. Queries are
limited to 4,096 characters and 32 levels of nested parentheses.

## Grammar

```ebnf
query      = [ "." ] , [ name ] , { step } ;
step       = "." , name | "[" , selector , "]" | "{" , name , { "," , name } , "}" ;
selector   = "*" | [ "-" ] , DIGITS | condition ;
condition  = conjunction , { "or" , conjunction } ;
conjunction = comparison , { "and" , comparison } ;
comparison = "(" , condition , ")" | operand , op , literal ;
operand    = "@" | name , { "." , name } ;
op         = "==" | "!=" | ">=" | "<=" | ">" | "<" | "~" ;
literal    = QUOTED | WORD ;
name       = ( ASCII_LETTER | "_" ) , { ASCII_LETTER | ASCII_DIGIT | "_" | "-" } | QUOTED ;
```

`WORD` is a run of characters other than space, `)`, and `]`, read as an AXS
scalar. `and` and `or` are keywords only when followed by a space or `(`.
