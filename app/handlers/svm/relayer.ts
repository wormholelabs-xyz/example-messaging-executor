/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/relayer.json`.
 */
export type Relayer = {
  address: "Ax7mtQPbNPQmghd7C3BHrMdwwmkAXBDq7kNGfXNcc7dg";
  metadata: {
    name: "relayer";
    version: "0.1.0";
    spec: "0.1.0";
    description: "Created with Anchor";
  };
  instructions: [
    {
      name: "executeVaaV1";
      docs: [
        "This instruction returns the instruction for execution based on a v1 VAA",
        "# Arguments",
        "",
        "* `ctx` - `ExecuteVaaV1` context",
        "* `vaa_body` - Body of the VAA for execution",
      ];
      discriminator: [195, 77, 54, 118, 36, 151, 194, 202];
      accounts: [];
      args: [
        {
          name: "vaaBody";
          type: "bytes";
        },
      ];
      returns: {
        defined: {
          name: "ix";
        };
      };
    },
  ];
  types: [
    {
      name: "acctMeta";
      type: {
        kind: "struct";
        fields: [
          {
            name: "pubkey";
            docs: ["An account's public key."];
            type: "pubkey";
          },
          {
            name: "isSigner";
            docs: [
              "True if an `Instruction` requires a `Transaction` signature matching `pubkey`.",
            ];
            type: "bool";
          },
          {
            name: "isWritable";
            docs: [
              "True if the account data or metadata may be mutated during program execution.",
            ];
            type: "bool";
          },
        ];
      };
    },
    {
      name: "ix";
      type: {
        kind: "struct";
        fields: [
          {
            name: "programId";
            docs: ["Pubkey of the program that executes this instruction."];
            type: "pubkey";
          },
          {
            name: "accounts";
            docs: [
              "Metadata describing accounts that should be passed to the program.",
            ];
            type: {
              vec: {
                defined: {
                  name: "acctMeta";
                };
              };
            };
          },
          {
            name: "data";
            docs: [
              "Opaque data passed to the program for its own interpretation.",
            ];
            type: "bytes";
          },
        ];
      };
    },
  ];
};
