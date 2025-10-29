# Celestia-Recursion

A succinct proof of Celestia's entire consesnsus history, using recursive zk proofs.

## Scraper Tool
This repo includes a tool that will download the minimum number of blocks to verify an entire chain's consensus history, using [Tendermint Skipping Verification](https://medium.com/tendermint/everything-you-need-to-know-about-the-tendermint-light-client-f80d03856f98).

**How to use the scraper tool**:

```
# create a directory somewhere to save the headers as files
mkdir ~/.crs
cargo run --bin scraper -- --output-path ~/.crs --rpc-url https://YOUR-CELESTIA-TENDERMINT-RPC-URL.com/
```

On Celestia mainnet, we found that only 53 blocks are needed to verify the enetire chain from genesis to 8144463