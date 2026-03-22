; SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
;
; SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

; Java symbol extraction queries

; Classes
(class_declaration
  name: (identifier) @name) @definition

; Interfaces
(interface_declaration
  name: (identifier) @name) @definition

; Enums
(enum_declaration
  name: (identifier) @name) @definition

; Methods
(method_declaration
  name: (identifier) @name) @definition

; Constructors
(constructor_declaration
  name: (identifier) @name) @definition
