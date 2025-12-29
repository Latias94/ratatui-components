# Glow Parity Fixture

This file is used to compare `MarkdownView` against Glow rendering.

## Inline Styles

Normal text, *emphasis*, **strong**, ~~strikethrough~~, and `inline code`.

This is an autolink: <https://github.com/charmbracelet/glow>

This is a [link](https://github.com/frankorz) with destination.

Reference link: [glow][glow-ref]

[glow-ref]: https://github.com/charmbracelet/glow

Relative link (requires base URL support): [relative](./relative/path)

## Lists

- item 1
- item 2
  - nested item
  - nested item
    1. deeper ordered
    2. deeper ordered

Loose list (with paragraphs):

- item A

  second paragraph for item A.

- item B

## Task List

- [x] done
- [ ] todo

## Blockquote

> A quote with *emphasis*.
>
> - a list inside quote
> - and another item
>
> ```
> code inside quote
> ```

## Code Fences (language normalization)

```rs
fn main() {
    println!("hello");
}
```

```language-rust
fn language_prefix() {}
```

```{.rust}
fn brace_class() {}
```

## Tables

| Name | Value | Notes |
|:-----|------:|:------|
| foo  | 123   | left/center/right alignment |
| bar  | 456   | wraps when the terminal is narrow and the cell content is long |

## Images

![Glow](https://github.com/charmbracelet/glow)

![Relative](./images/logo.png)

## Footnotes

Footnote reference[^a] and another[^long].

[^a]: Short footnote definition.
[^long]: A longer footnote definition that should wrap and keep indentation.

## Horizontal Rule

---

## HTML (fallback)

<kbd>Ctrl</kbd> + <kbd>C</kbd>

## Math (fallback)

Inline math: $a^2 + b^2 = c^2$

Display math:

$$
\int_0^1 x^2 dx
$$
