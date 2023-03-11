import task from "terrariums";

task(async ({ deployer, signer, refs }) => {
  //account
  deployer.buildContract("warp-account");
  deployer.optimizeContract("warp-account");

  const id = await deployer.storeCode("warp-account");
  await new Promise((resolve) => setTimeout(resolve, 3000));

  // needed for terrarium to successfully import warp account contract
  // we will not actually use this account, we only create account by calling warp-controller
  // await deployer.instantiate("warp-account", { owner: "terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95" })

  //controller
  deployer.buildContract("warp-controller");
  deployer.optimizeContract("warp-controller");

  await deployer.storeCode("warp-controller");
  await new Promise((resolve) => setTimeout(resolve, 3000));
  //
  const instantiateMsg = {
    warp_account_code_id: id,
    creation_fee: "5",
    cancellation_fee: "5",
    minimum_reward: "10000",
    template_fee: "10000000",
    t_max: "86400",
    t_min: "86400",
    a_max: "10000",
    a_min: "10000",
    q_max: "10",
  };

  await deployer.instantiate("warp-controller", instantiateMsg, {
    admin: signer.key.accAddress,
  });

  refs.saveRefs();
});
