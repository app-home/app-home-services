# Data Model: Admin

## Overview

The admin bounded context provides user management capabilities for
administrator users. It reuses the existing `users` table with an added
`role` column.

## Table: `users` (admin-relevant columns)

| Column         | Type                     | Constraints                  |
|----------------|--------------------------|------------------------------|
| `id`           | `uuid`                   | PK                           |
| `username`     | `varchar(255)`           | nullable, unique             |
| `email`        | `varchar(255)`           | NOT NULL, unique             |
| `display_name` | `varchar(255)`           | NOT NULL                     |
| `role`         | `varchar(20)`            | NOT NULL, default 'user'     |
| `auth_provider`| `varchar(50)`            | NOT NULL, default 'local'    |
| `created_at`   | `timestamptz`            | NOT NULL                     |
| `updated_at`   | `timestamptz`            | NOT NULL                     |

## Domain

| Value Object | Values                        |
|--------------|-------------------------------|
| `Role`       | `"user"` or `"admin"`         |
