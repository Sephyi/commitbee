; SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
;
; SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

; Ruby symbol extraction queries

; Classes
(class
  name: (constant) @name) @definition

; Modules
(module
  name: (constant) @name) @definition

; Methods
(method
  name: (identifier) @name) @definition

; Singleton methods (self.method_name)
(singleton_method
  name: (identifier) @name) @definition
