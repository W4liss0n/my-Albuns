# Embedded color profiles

The three profiles below are distributed unmodified by the International Color Consortium:

- `sRGB2014.icc`: [source](https://registry.color.org/rgb-registry/profiles/sRGB2014.icc), `3,024` bytes, SHA-256 `384b832de3412066743b52a75ee906b6fb9fb8d9e09e936fc2c43223815c6e0a`;
- `sRGB_v4_ICC_preference.icc`: [source](https://registry.color.org/rgb-registry/profiles/sRGB_v4_ICC_preference.icc), `60,960` bytes, SHA-256 `83174717332326ddc198d9df188a4daec27b8979ba152cebbfc470c793d0bb11`;
- `sRGB_v4_ICC_preference_displayclass.icc`: [source](https://registry.color.org/rgb-registry/profiles/sRGB_v4_ICC_preference_displayclass.icc), `60,988` bytes, SHA-256 `f54b145a18e4b12112750e672f1c79cac9347dc8403da3955e7f74a352816a21`.

The ICC permits these profiles to be copied and distributed at no charge, provided that the files and their embedded copyright notices remain unchanged. The profiles are embedded verbatim, and the ICC name is not used to promote the product.

O perfil legado HP/Microsoft `sRGB IEC61966-2.1`, fornecido pelo Windows como
`sRGB Color Space Profile.icm`, também é reconhecido: `3.144` bytes, SHA-256
`2b3aa1645779a9e634744faf9b01e9102b0c9b88fd6deced7934df86b949af7e`.
O aplicativo contém somente seu tamanho e digest; esse perfil não é incorporado
nem redistribuído. Os testes Windows usam a cópia do sistema e verificam ambos
os valores. A [documentação da Microsoft](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/color-management-for-still-image-devices)
identifica o arquivo como perfil padrão para fontes sRGB.
