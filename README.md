# recursive-sync-tia
Verify the entire blockchain's consensus history, from genesis to network head, with one succinct proof.

**Use Cases**:
- Data bridging
  - validate Celestia blob inclusion for rollups on *any blockchain*, even ones without [Blobstream](https://docs.celestia.org/how-to-guides/blobstream).
- Fast-sync light clients
  - augment light nodes to instantly find a trustworthy network head to start backwards syncing.

## Scraper Tool
This repo includes a tool that will download the minimum number of blocks to verify an entire chain's consensus history, using [Tendermint Skipping Verification](https://medium.com/tendermint/everything-you-need-to-know-about-the-tendermint-light-client-f80d03856f98).

**How to use the scraper tool**:

```
# create a directory somewhere to save the headers as files
mkdir ~/.crs
cargo run -p scraper --bin scraper -- --output-path ~/.crs --rpc-url https://YOUR-CELESTIA-TENDERMINT-RPC-URL.com/
```

On Celestia mainnet, we found that only 53 blocks are needed to verify the enetire chain from genesis to 8144463

## Accumulating Versioned Verification Keys

SP1 recursion has [been known](https://github.com/S1nus/celestia-recursive-sync/issues/3) to break on upgrade boundaries (e.g, a new version of SP1 verifying a proof from an older version). As a fix, this repo supports using the groth16 verifier as an intermediary.

We accumulate a history of changes to the groth16 verification key in the public inputs of the proof, so anyone can verify all changes to the long-running chain of proofs.
