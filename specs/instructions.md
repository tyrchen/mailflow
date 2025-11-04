# Instructions

## Initial idea

I want to use SES, SQS, S3 and lambda function to build a email dispatching system (mailflow) that users could send emails to specific recipients that an app will receive and process. The system shall be able to receive emails from SES and extract the data (email info and attachments) and then based on the receipent address (e.g. <_app1@acme.com>, <_app2@acme.com>, etc.), the system shall send the data to the designated SQS queue (mailflow-<app-name>).

mailflow will also retrieve the data from a SQS queue (mailflow-outbound). App will send the response email to the queue after finishing the processing. mailflow shall send the response email to the sender based on the email info.

please think ultra hard, consider all the possible scenarios and edge cases, and then write a high quality product spec for this under ./specs/0001-spec.md.

## design spec

Based on ./specs/0001-spec.md, please write a design spec for the system under ./specs/0002-design.md. Make sure you use Rust ecosystem and email related crates (mail-parser, lettre) could be referenced in ./vendors/. Rust practices should be followed. For deployment, please use pulumi. Do not put detailed code, only focus on the arch and high level design (high level interfaces). Write down proper mermaid diagrams for arch and data flow. Put the design spec in ./specs/0002-design.md.

## implementation plan

Based on ./specs/0002-design.md, please write an implementation plan for the system under ./specs/0003-implementation-plan.md.

## Fix email issue

See spec ./specs/0001-spec.md about this tool and read the code to understand the detailed implementation. I've sent a test email successfully:

```bash
aws ses send-email --from test@yourdomain.com --destination ToAddresses=_app1@yourdomain.com --message "Subject={Data=Test},Body={Text={Data=Hello}}" --region us-east-1 --profile your-aws-profile
{
    "MessageId": "0100019a405ffd47-b9b99d7d-27fe-4a7c-9471-8e8aa29915f0-000000"
}
```

However when I try to get it from sqs, I got nothing:

```bash
➜ aws sqs receive-message --queue-url https://sqs.us-east-1.amazonaws.com/123456789012/mailflow-app1-dev --max-number-of-messages 1 --region us-east-1 --profile your-aws-profile

```

Use aws cli to find the logging in lambda / ses / sqs to find out the root cause, think ultra hard and put a detailed design/implementation plan to fix this issue under ./specs/fixes/0001-fix-email-issue.md. After that, implement the fix and test it to make sure things are working as expected.

## Fix email attachment metadata issue

Think ultra hard make a good design and implementation plan in ./specs/fixes for attachment metadata parsing. The app which pull the SQS shall be able
to process attachment easily. Once you finished, implement it

## Review the code

Review the code carefully against ./specs/0001-spec.md, think ultra hard on these:

- any missing features or any issues through code review
- any potential security issues
- any improvements could be made for usability to upstream apps and extensibility
- code quality improvements: use typed-builder for structs that needs to be constructed many times and need to provide many values. Use From/TryFrom/FromStr for type conversion. Use best practices for Rust. Extract literals to constants, use envar / config for things that should be configurable, etc.
- better error handling and observability

Once you finished, put a detailed review report and implementation plan under ./specs/reviews/0001-system-review.md.

Then implement it entirely and test thoroughly to make sure things are working as expected.

## further review

based on @specs/0001-spec.md and @specs/reviews/0001-system-review.md and existing code make sure you identify unfinished work, think ultra hard,
properly design them and put a implementation plan on @spec/reviews/0002-2nd-review.md. Then implement the entire plan. Use @specs/0004-verification-plan.md to verify the system work as expected.

## update plan

we can skip malware scanning, instead let's make sure we do file type inspection and only allow a set of attached files to be stored (could be
configured in envar for lambda function, default: all images, pdf, well known doc types). Update the plan and finish all phase 3 tasks and verify

## integration and e2e test plan

based on @specs/0001-spec.md think ultra hard and build a concrete integration and e2e test plan under @specs/0005-integration-and-e2e-test-plan.md. The e2e should be written in python using uv. Code should be in ./e2e.

## integration tests

read existing rust code carefully, based on @specs/0005-integration-and-e2e-test-plan.md, think ultra hard and build all the integration tests, verify they compile and all tests pass.

## e2e tests

now implement all python e2e tests based on @specs/0005-integration-and-e2e-test-plan.md. Make sure you use uv and add targets to run e2e test in
@Makefile. Once you finished one e2e run and make sure it work as expected before working on next one

## A new lambda function for the UI dashboard

based on @specs/0001-spec.md and @specs/reviews/0001-system-review.md and existing code make sure you understand the existing work, think ultra hard and properly design a dashboard that admin could use to manage the system:

- look at the system status, including basic metrics and charts
- see the detailed logs from different resources and inspect data in the queues
- send test emails to the system to verify the system is working as expected

Feel free to add anything you feel important to the PRD, but keep this dashboard focused, no need to consider monitor/alerting, as those should be processed via otel and a different dashboard (e.g. datadog). For the system design, consider the following:

- move the code base to a multi-crate one, with the new lambda function for UI dashboard in a separate crate. For common code, use a shared crate.
- the new lambda function for UI dashboard should handle all API related work, use Tokio/Axum/aws sdk to build it. Use jwt to protect the API endpoints. Infra code shall pass a JWKS envar to the lambda function, containing the JWKS json data. The jwks file is in ./infra/.jwks.json (gitignored).
- the UI dashboard should be built with refine/antd/tailwindcss/typescript/vite/react. It should build assets and deploy to a s3 bucket and use cloudfront to serve it.

The design should focus on high level interfaces and data flow, no need to consider the implementation details.

Write a detailed PRD, design spec and implementation plan in @specs/0007-dashboard.md.
