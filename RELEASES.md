## Version 0.5.0
- _**Breaking Change**_: Function `expect_json_body` was renamed to `expect_json_body_obj`. 
- _**Breaking Change**_: Function `return_json_body` was renamed to `return_json_body_obj`. 
- A new function `expect_json_body` was added which takes a `serde_json::Value` as an argument. 
- A new function `return_json_body` was added which takes a `serde_json::Value` as an argument. 
- Improved documentation.

## Version 0.4.5
- Improved documentation.
- Added a new function `base_url` to the `MockServer` structure.