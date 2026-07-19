# Data Model: User Profiles

## Overview

The user profiles bounded context manages profile information (avatar, bio) for
authenticated users. Each user has exactly one profile record, auto-created on
first access.

## Table: `user_profiles`

| Column      | Type                     | Constraints                  |
|-------------|--------------------------|------------------------------|
| `user_id`   | `uuid`                   | PK, FK → users(id)           |
| `avatar_url`| `varchar(500)`           | nullable                     |
| `bio`       | `varchar(2000)`          | nullable                     |
| `updated_at`| `timestamptz`            | NOT NULL                     |

## Domain

| Value Object   | Constraint                      |
|----------------|---------------------------------|
| `AvatarUrl`    | ≤ 500 chars, valid URL format   |
| `Bio`          | ≤ 2000 chars                    |
