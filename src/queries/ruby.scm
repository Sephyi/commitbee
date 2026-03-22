; SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
;
; SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

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
