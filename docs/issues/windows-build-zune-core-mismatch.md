# Issue: Windows build fails with zune-core/zune-jpeg type mismatch

## Summary
Windows CI fails building the `image` crate with trait/type mismatches between
`zune_core` and `zune_jpeg`, indicating two different `zune-core` sources are
compiled simultaneously.

## Error signature
The failures show mismatched `ZByteReaderTrait` bounds and `DecoderOptions`
types coming from two different `zune_core` crates.

## Hypothesis
We were overriding `zune-jpeg` and `zune-core` to a git source while `image`
still pulled `zune-core` from crates.io, leaving two sources in the graph.

## Proposed fix
Remove the git patches so `image`, `zune-jpeg`, and `zune-core` resolve to the
same crates.io sources.

## Branch
`issue/windows-zune-core-mismatch`
