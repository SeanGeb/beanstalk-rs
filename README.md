# beanstalk-rs: got beans?

A memory-safe work queue backwards-compatible with beanstalkd.

## Features

* High compatibility with the original beanstalkd.
* High performance thanks to a modern, multi-threaded, async design.
* Assured memory safety thanks to Rust.

## Planned features

* Queue introspection: inspect jobs in the queue (not just the first in the queue).
* Queue management: change job priorities, or move them between states, based on the job content.
  * Supporting common data formats, including JSON and YAML, or plain old regex.
* Durable queues with a WAL and defined durability properties.
* Replication to another beanstalkd or `beanstalk-rs` server.
* Distributed tracing support with OpenTelemetry, as an optional feature.

## Notice

All files in this repo, unless otherwise indicated, are provided under the following notice:

> Copyright (C) 2024 Sean Gebbett
>
> This program is free software: you can redistribute it and/or modify
> it under the terms of the GNU General Public License as published by
> the Free Software Foundation, either version 3 of the License, or
> (at your option) any later version.
>
> This program is distributed in the hope that it will be useful,
> but WITHOUT ANY WARRANTY; without even the implied warranty of
> MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
> GNU General Public License for more details.
>
> You should have received a copy of the GNU General Public License
> along with this program. If not, see <https://www.gnu.org/licenses/>.
