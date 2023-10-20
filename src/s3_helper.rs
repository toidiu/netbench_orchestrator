use aws_sdk_s3 as s3;

use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::operation::get_object::GetObjectError;
use aws_sdk_s3::operation::get_object::GetObjectOutput;
use aws_sdk_s3::operation::put_object::PutObjectError;
use aws_sdk_s3::operation::put_object::PutObjectOutput;

pub async fn download_object(
    client: &s3::Client,
    bucket_name: &str,
    key: &str,
) -> Result<GetObjectOutput, SdkError<GetObjectError>> {
    client
        .get_object()
        .bucket(bucket_name)
        .key(key)
        .send()
        .await
}

pub async fn upload_object(
    client: &s3::Client,
    bucket_name: &str,
    body: s3::primitives::ByteStream,
    key: &str,
) -> Result<PutObjectOutput, SdkError<PutObjectError>> {
    client
        .put_object()
        .bucket(bucket_name)
        .key(key)
        .body(body)
        .send()
        .await
}
