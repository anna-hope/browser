# Octo

This is a work-in-progress web browser, based on the https://browser.engineering book.
I am building it as part of my batch at the [Recurse Center (RC)](https://recurse.com).
The browser in that book is built in Python, but I am building it in Rust, because Rust currently brings me
more joy than Python.

I hope to finish it before my RC batch ends, but partly because I chose to do it in Rust, I have no idea if I will.

## Build and run

The browser can currently run only from the command line. It is therefore not very good as a browser.

`cargo run -- https://example.org`

### Test

`cargo test`

## Miscellaneous

Because I am doing this for joy and not for money, in this codebase,
I am trying to strike a balance between correctness, cleanliness, effective patterns, monstrous
unhinged functional code-golf one-liners that I am writing because I can, and cutting corners to not fall
hopelessly behind the rest of the group I am building this with.
Most of them are using Python, and are therefore not spending extra
fun time yak shaving Iterator implementations or going on side quests to see if there is a way I can reduce allocations
without upsetting the borrow checker. One person is using Go, which seems like a prudent choice as well.

(Another is building his browser as an Electron app in Node.js, which means his browser runs in a browser.
There is nothing about that I do not love.)

### Why Octo?

Because this 🐙 emoji is cute. 
