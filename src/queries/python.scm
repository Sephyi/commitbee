; SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
;
; SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

(function_definition name: (identifier) @name) @definition
(class_definition name: (identifier) @name) @definition
(decorated_definition definition: (function_definition name: (identifier) @name)) @definition
(decorated_definition definition: (class_definition name: (identifier) @name)) @definition
