# zuri-rs

This project is a Just-In-Time (JIT) compiler for the [Zuri](https://github.com/zuri-lang/zuri) programming language. This project aims to provide a faster runtime for Zuri programs while remaining 100% backwards-compartible with the C implementation and will become the defacto implementation when it becomes stable enough.

This project will also support more operating systems and more processor architectures compared to the C implementation.

## Documentation

[`docs/`](docs) holds everything written about the language, and [the book](docs/book/src/SUMMARY.md) is the main text. Read it on GitHub as it is, or render it locally:

```console
$ cargo run-docs
```

## Roadmap

- [x] 100% compatibility with the original Zuri language
- [x] Just-In-Time compiler
- [ ] Package manager
- [ ] Self-Hosted repository server
- [ ] C and Rust compartible FFI Interop


## AI Involvement

AI writes code. This is year 2026, and that's no longer a fact that can be denied. However, how competent it is at that is debatable, and how competent I am at using it to write code efficiently is even a much more debatable topic as I myself would score my AI skills well below 5/100. I'm terrible at it, and there's no hiding that. I just almost never was able to make it achieve my goal. That's a fact that's very obvious from the commit history.

The involvement of AI in this project is a topic I believe must be clearly documented as it will serve as a precursor to two important decisions to contributing code to this repository I'll be getting to shortly.

After the addition of the preliminary JIT work from an earlier rudimentary attempt at the JIT in the predecessor repository, I decided to enlist the help of `Claude Sonnet/Opus 5` in the project to fine-tune the JIT into a first class JIT system as the old system was very far from where I wanted it to be (which was exactly why I discarded the old project). 

Starting from commit `eed08e1` to `1f45d18`, I struggled to make Claude Sonnet/Opus 5 make significant fixes that will help the JIT performance significantly. However, with each passing commit, the performance of the JIT system continued to degrade even worse than my original implementation in many cases. Every recommendation from Claude Sonnet/Opus 5 which promised significant performance benefits did the opposite -- They degraded performance (yeah and the `--` hell everywhere was beginning to get on my nerves, so here's me teaching Claude Sonnet/Opus 5 how to use it correctly; that is if it ever cared to read the README.md or its own CLAUDE.md at all despite many explicit instructions).

At that point, I downgraded Claude Sonnet/Opus 5 involvement to simply documenting code (because that's the part of software development I find tedious -- Yes, I am that lazy. Just check the older versions of Zuri which was formerly called Blade Programming Language).

Starting from commit `6d31444`, I enlisted Claude Sonnet 5 this time (I don't have any money to waste any more at this point) to clean up Claude's own mess that it created from earlier works where it bastardized my repository with ridiculous comments and standard library documentations that would make any human reader feel like throwing up. For this simple task, it failed woefully! However, because I'm that lazy, obviously AI generated documentation was better than no documentation so I kept it in that capacity.

After commit `2917496`, I hit a real mental blocker so I enslisted the help of `Claude Opus 5` this time again with a really laid out prompt. So starting from commit `bcf1dbd` to `1007c4d`, I upgraded AI to coding tasks again. My experience, not much different from the earlier version. However, this time, it seemed to follow my instructions much more worse and literally added to my mental block. At this point, I just gave up on it. Went to the kitchen to make myself some good noodles and uninstalled Claude Code altogether. But after a good plate of noodles, I installed it back. So I kept using it up until commit `488d3b2`.

> I literarly have three commits where I vented my frustruaion. Commit `d91bc4b9` with message `I don't even know how we got here` and `767dee2` with message `i don't know what to call this state, but it feels like time wasted on claude sonnet 5 which ended up making my code extremely more complex for not even up to 1% performance boost`, and `9e37e3c` with message `some multihour claude session to optimize the JIT that ended up at a performance exactly where I left it and in some cases slower`.
> If you understand this codebase enough and want to get a good laugh, just check out those commits. It was like someone was paying Claude to do exactly opposite of what I asked of it. They sank performance so bad, it was 1.5x, 1.01x, and 3x slower than my original implementation respectively.

After this issue, Claude became permanently relegated to a planning, documentation and tests writing agent.

Which brings us to one important rule which I'm going to iterate here and in the contributing secion at the risk of sounding like a broken record:

> **IMPORTANT!**
> 
> If you're contributing code that has AI footprint, ensure to:
> 1. Before starting your change, run all of the benchmarks under an identified and controlled system load and take note of the performance.
> 2. Run the tests and ensure no regression using the command `cargo test --test zuri`.
> 3. Run all the benchmarks again under similar load after your changes to ensure that none of the benchmarks regressed.

## License

This project shares the same license as [Zuri](https://github.com/zuri-lang/zuri).
