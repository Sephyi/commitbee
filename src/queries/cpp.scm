; SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
;
; SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

; C++ symbol extraction queries

; Functions
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition

; Classes
(class_specifier
  name: (type_identifier) @name) @definition

; Structs
(struct_specifier
  name: (type_identifier) @name) @definition

; Enums
(enum_specifier
  name: (type_identifier) @name) @definition
