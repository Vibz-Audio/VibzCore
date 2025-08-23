# Vibz

Vibz is a DAW (Digital Audio Workstation) written primarily in Rust. The goal of this project
is to create a DAW that centralizses it's workflow around Keyboard input, allowing for fast iterations and creativity.

Vibz should be cross-platform, running on Windows, Linux and MacOS.
Vibz need to be small and fast, allowing it to run on low-end hardware.

# Architecture

/**********\***********/

- +----------------+ \*/
- | User Input | \*/
- +----------------+ \*/
-          |           */
-          v           */
- +----------------+ \*/
- | UI Layer | \*/
- +----------------+ \*/
-          |           */
-          v           */
- +----------------+ \*/
- | Core Logic | \*/
- +----------------+ \*/
-          |           */
-          v           */
- +----------------+ \*/
- | Audio Engine | \*/
- +----------------+ \*/
  /**********\***********/

# Pipeline Audio

Clip -> Effect -> Output
