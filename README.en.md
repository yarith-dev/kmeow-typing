# Kmeow Engine

**English** | [ភាសាខ្មែរ](README.md)

Type Khmer with Latin letters, such as **`sievphov` → សៀវភៅ**. Kmeow suggests words as you type and also supports English. Suggestions work offline.

## How suggestions work

Kmeow looks for words using the spellings you saved and its pronunciation rules.

| Label | What it means |
| --- | --- |
| **Match** | What you typed already matches a complete word. |
| **Completion** | A longer spelling is being offered. |
| **Correction** | A nearby spelling is offered in case you made a typo. |
| **Select explicitly** | Choose this word yourself; it needs your confirmation. |

A suggestion can have more than one label. Several words can share a spelling, so you may see multiple choices.

## Why one word comes first

For Khmer, the order favors:

1. Complete matches.
2. Words that continue the letters you typed.
3. Possible typo corrections.

For example, typing **`broya`** offers:

| Word | Spelling | Why |
| --- | --- | --- |
| **ប្រយ័ត្ន** | `broyat` | Keeps all of `broya` and adds `t`. |
| **ប្រសា** | `brosa` | Changes a letter you already typed. |

That is why **ប្រយ័ត្ន comes first**.

Within these groups, common words usually appear earlier. Surrounding words can also help choose between suggestions. Less common words remain available, but some require explicit selection.

English has its own correction behavior. With English correction enabled, **`im` can offer `I'm`**. If you save `im` as your own English word, it stays literal.

## Choose a suggestion

Click a word or press Enter to insert it. Use the arrow keys to change your choice.

In **Typing options**, choose **Auto** to let Space insert an eligible match or completion. **Manual** keeps what you typed when you press Space. Choices marked **Select explicitly** need a click or Enter. Shift+Space types a literal space.

## Make it yours

For full Khmer sentences, choose **Khmer → Converter** on the Typing page. Type Romanized text in the first box and copy the Khmer result from the second. Place the cursor in a word in either box to choose another suggestion. Your choices stay with the sentence as you edit, with separate Undo and Redo controls.

In **Word list**, open a word and choose **Edit word**. You can:

- Add your preferred Romanized spellings.
- Change how common the word is.
- Give a whole word a short input, such as `nh` for ខ្ញុំ.
- Hide that word for a particular input.
- Turn off its automatically generated spellings while keeping saved ones.

Use **Sound map → Read-As** to describe a word's pronunciation using Khmer letters. Custom spellings stay separate from generated readings.

For multiple readings, separate the Khmer spellings with commas. All readings insert the original word.

Word changes save to the shared dictionaries. Selecting a suggestion while typing does not add a new spelling to the word list.

## Try it

With **Node.js 22 or newer** installed, run:

```sh
npm ci
npm run build
npm run manage
```

Open [localhost:3001](http://127.0.0.1:3001): **Typing** to try suggestions, **Sound map** to explore the rules, and **Word list** to manage words.

For app integration, see the [API reference](src/public/types.ts). Build and test commands are in [package.json](package.json).

## License

[MIT](LICENSE). See the [dictionary license](DICTIONARY-LICENSE.md) and [dependency notices](NOTICES.txt) for details.
