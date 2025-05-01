# Oxiadm Administration Tool

Oxiadm is the administration component for the Oxifed ActivityPub server. It provides tools for managing ActivityPub entities, WebFinger profiles, and system configuration.

## Features

- Management of different types of ActivityPub objects:
  - Person actors
  - Notes
  - Activities (Create, Follow, Like, Announce)
  - Custom ActivityPub object types
- WebFinger profile management
- AMQP messaging integration for ActivityPub events
- MongoDB persistence for ActivityPub objects

## Configuration

The tool requires the following environment variables:

- `MONGODB_URI`: MongoDB connection URI
- `MONGODB_DATABASE`: MongoDB database name
- `RABBITMQ_URI`: RabbitMQ connection URI

## Usage

### Managing ActivityPub Objects

Oxiadm supports various types of ActivityPub objects through a flexible system that can be extended to accommodate new object types in the future. The system uses the `ActivityObject` trait to standardize behavior across different object types.
Administration CLI tool for Oxifed, designed with a structure similar to Solaris commands.

## Usage

### Creating Profiles

Create a new JrdResource profile:
