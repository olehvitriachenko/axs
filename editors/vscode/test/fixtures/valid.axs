# Application settings
server:
  host: localhost
  port: 3000
  enabled: true
  title: Олег 🚀 says true
skills:
  - Rust
  - "UI:"
  -
    - nested
categories:
  - UI:
      - MUI
      - Tailwind
    weight: 2
users{id,name}:
  1,Oleh
  2,Alex
owner: {login: oleh,id: 1}
notes: |
  First line
    indented
limits:
  maxUploadBytes: 9223372036854775807
  ratio: 1.0
  tags: []
