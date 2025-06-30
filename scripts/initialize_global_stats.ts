// scripts/initialize_global_stats.ts
import * as anchor from "@coral-xyz/anchor";

const run = async () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.WpmwarProgram;

  const [globalStatsPDA] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("global_stats")],
    program.programId
  );

  await program.methods.initializeGlobalStats().accounts({
    admin: provider.wallet.publicKey,
    globalStats: globalStatsPDA,
    systemProgram: anchor.web3.SystemProgram.programId
  }).rpc();

  console.log("✅ Global stats initialized:", globalStatsPDA.toBase58());
};

run().catch(err => {
  console.error(err);
});
