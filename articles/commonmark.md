+++
title = "CommonMark Cheat Sheet"
edit_access = "Authenticated"
view_access = "Authenticated"
+++
See also:
- [CommonMark help](https://commonmark.org/help/)
- [Pulldown-cmark Cheat Sheet](https://pulldown-cmark.github.io/pulldown-cmark/cheat-sheet.html)

## Inline

| Markdown                           | Result                           |
|------------------------------------|----------------------------------|
| `*italic*`                         | *italic*                         |
| `**bold**`                         | __bold__                         |
| `- Unordered List`                 | <ul><li>Unordered List</li></ul> |
| `- Ordered List`                   | <ol><li>Ordered List</li></ol>   |
| <code>\`inline-code\`</code>       | `inline-code`                    |
| `~~Strikethrough~~`                | ~~Strikethrough~~                |
| `[[commonmark]]`                   | [[commonmark]]                   |
| `[Link](https://example.com/)`     | [Link](https://example.com/)     |
| `![Alt Text](/assets/smolwik.svg)` | ![Alt Text](/assets/smolwik.icon.svg) |

## Blocks

### Headers

Header Level 1
----
```markdown
Header Level 1
----
```

Header Level 2
====

```markdown
Header Level 2
====
```

### Header Level 3

```markdown
### Header Level 3
```

### Code Blocks

    ```markdown
    Fenced
    ```

```markdown
    Indented
    Four Spaces
```

```markdown
Fenced
```

    Indented
    Four Spaces

### Block Quotes

```markdown
> Block Quote
```

> Block Quote

## Extensions
These are unofficial extensions to the CommonMark spec. For more details about how these particular extensions are
implemented, refer to the [pulldown-cmark specification](https://pulldown-cmark.github.io/pulldown-cmark/specs).

## Tables

[pulldown-cmark guide](https://pulldown-cmark.github.io/pulldown-cmark/specs/table.html)

|Column A|Column B|Column C|
|-------:|:------:|:-------|
|Right   |Center  |Left    |
|Aligned |Aligned |Aligned |

```markdown
|Column A|Column B|Column C|
|-------:|:------:|:-------|
|Right   |Center  |Left    |
|Aligned |Aligned |Aligned |
```

## Footnotes

[pulldown-cmark guide](https://pulldown-cmark.github.io/pulldown-cmark/specs/footnotes.html)

The sky is blue.[^1]
The sky is green.[^2]

[^1]: Look Up

```markdown
The sky is blue.[^1]
The sky is green.[^2]

[^1]: Look Up
```

## Math

[pulldown-cmark guide](https://pulldown-cmark.github.io/pulldown-cmark/specs/math.html)

*Note*: Additional processing are required to format the mathematical formulas; for example,
[MathJax](https://www.mathjax.org/).

Inline: $5x + 2 = 17$

$$5x + 2 = 17$$

```markdown
Inline: $5x + 2 = 17$

$$5x + 2 = 17$$
```
