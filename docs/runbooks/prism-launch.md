# Prism launch

Status: scaffold; exact M0 inputs are not yet recorded.

1. Use Prism Launcher for authentication, entitlement, Minecraft installation,
   instance management, and pack acquisition.
2. Create an isolated instance pinned to Minecraft 26.1.2 and the Java runtime
   required by its verified metadata.
3. Configure only the reviewed Ravel bootstrap produced by the selected ADG-1
   design. Do not install another mod loader.
4. Keep launcher data, tokens, game files, and official fixtures outside the
   repository.
5. Record sanitized Prism, Java, Minecraft, Ravel, platform, and launch-setting
   versions in the evidence manifest.
6. Verify from the process and instance inventory that NeoForge, Fabric, Forge,
   Quilt, and other loaders are absent.

M0-W6 replaces this scaffold with exact clean-machine steps. ADG-5 records
whether stock Prism is sufficient or a small reviewed patch is needed.
