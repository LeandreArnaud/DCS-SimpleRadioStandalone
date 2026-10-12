# Branching strategy

## Release
This branch is mostly the current release, and can receive fixes, but not really new features directly.

## Beta
This is the branch that acts as a buffer for larger, more risky changes.
A tag can be made from beta, and necessary iterations for fixes can go there.
Once it has stabilized, it will be moved back to release (often a pave over).

## Master
This is where the main development effort happens.
Depending on the level of risk for the upcoming changes, it will go either straight to release, or beta.

## Merge directions

```
master  -> [beta, release]
beta    -> release
release -> master

tags off of release or beta.
```