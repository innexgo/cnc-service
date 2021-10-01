use std::error::Error;

use super::Config;
use super::Db;
use auth_service_api::request;
use auth_service_api::response;

use super::api_key_service;
use super::db_types::*;
use super::email_service;
use super::parent_permission_service;
use super::password_reset_service;
use super::password_service;
use super::user_data_service;
use super::user_service;
use super::utils;
use super::verification_challenge_service;

use mail_service_api::client::MailService;
use mail_service_api::response::MailError;

static FIFTEEN_MINUTES: u64 = 15 * 60 * 1000;

fn report_internal_err<E: std::error::Error>(e: E) -> response::AuthError {
  utils::log(utils::Event {
    msg: e.to_string(),
    source: e.source().map(|e| e.to_string()),
    severity: utils::SeverityKind::Error,
  });
  response::AuthError::Unknown
}

fn report_postgres_err(e: tokio_postgres::Error) -> response::AuthError {
  utils::log(utils::Event {
    msg: e.to_string(),
    source: e.source().map(|e| e.to_string()),
    severity: utils::SeverityKind::Error,
  });
  response::AuthError::InternalServerError
}

fn report_mail_err(e: MailError) -> response::AuthError {
  let ae = match e {
    MailError::DestinationBounced => response::AuthError::EmailBounced,
    MailError::DestinationProhibited => response::AuthError::EmailBounced,
    _ => response::AuthError::EmailUnknown,
  };

  utils::log(utils::Event {
    msg: ae.as_ref().to_owned(),
    source: Some(format!("email service: {}", e.as_ref())),
    severity: utils::SeverityKind::Error,
  });

  ae
}

async fn fill_user(
  _con: &mut tokio_postgres::Client,
  user: User,
) -> Result<response::User, response::AuthError> {
  Ok(response::User {
    user_id: user.user_id,
    creation_time: user.creation_time,
  })
}

async fn fill_user_data(
  _con: &mut tokio_postgres::Client,
  user_data: UserData,
) -> Result<response::UserData, response::AuthError> {
  Ok(response::UserData {
    user_data_id: user_data.user_data_id,
    creation_time: user_data.creation_time,
    creator_user_id: user_data.creator_user_id,
    name: user_data.name,
  })
}

async fn fill_api_key(
  _con: &mut tokio_postgres::Client,
  api_key: ApiKey,
  key: Option<String>,
) -> Result<response::ApiKey, response::AuthError> {

  Ok(response::ApiKey {
    api_key_id: api_key.api_key_id,
    creation_time: api_key.creation_time,
    creator_user_id: api_key.creator_user_id,
    api_key_data: match api_key.api_key_kind {
      request::ApiKeyKind::Valid => response::ApiKeyData::Valid {
        duration: api_key.duration,
        key,
      },
      request::ApiKeyKind::Cancel => response::ApiKeyData::Cancel,
    },
  })
}

async fn fill_email(
  con: &mut tokio_postgres::Client,
  email: Email,
) -> Result<response::Email, response::AuthError> {

  let verification_challenge =
    verification_challenge_service::get_by_verification_challenge_key_hash(
      con,
      &email.verification_challenge_key_hash,
    )
    .await
    .map_err(report_postgres_err)?
    .ok_or(response::AuthError::VerificationChallengeNonexistent)?;

  Ok(response::Email {
    email_id: email.email_id,
    creation_time: email.creation_time,
    creator_user_id: email.creator_user_id,
    verification_challenge: fill_verification_challenge(con, verification_challenge).await?,
  })
}

async fn fill_parent_permission(
  con: &mut tokio_postgres::Client,
  parent_permission: ParentPermission,
) -> Result<response::ParentPermission, response::AuthError> {

  let verification_challenge = match parent_permission.verification_challenge_key_hash {
    Some(verification_challenge_key_hash) => {
      let verification_challenge =
        verification_challenge_service::get_by_verification_challenge_key_hash(
          con,
          &verification_challenge_key_hash,
        )
        .await
        .map_err(report_postgres_err)?
        .ok_or(response::AuthError::VerificationChallengeNonexistent)?;
      Some(fill_verification_challenge(con, verification_challenge).await?)
    }
    _ => None,
  };

  Ok(response::ParentPermission {
    parent_permission_id: parent_permission.parent_permission_id,
    creation_time: parent_permission.creation_time,
    user_id: parent_permission.user_id,
    verification_challenge,
  })
}

async fn fill_password(
  con: &mut tokio_postgres::Client,
  password: Password,
) -> Result<response::Password, response::AuthError> {

  let password_reset = match password.password_reset_key_hash {
    Some(password_reset_key_hash) => {
      let password_reset =
        password_reset_service::get_by_password_reset_key_hash(con, &password_reset_key_hash)
          .await
          .map_err(report_postgres_err)?
          .ok_or(response::AuthError::PasswordResetNonexistent)?;
      Some(fill_password_reset(con, password_reset).await?)
    }
    _ => None,
  };

  Ok(response::Password {
    password_id: password.password_id,
    creation_time: password.creation_time,
    creator_user_id: password.creator_user_id,
    password_reset,
  })
}

async fn fill_password_reset(
  _con: &tokio_postgres::Client,
  password_reset: PasswordReset,
) -> Result<response::PasswordReset, response::AuthError> {
  Ok(response::PasswordReset {
    creation_time: password_reset.creation_time,
  })
}

async fn fill_verification_challenge(
  _con: &tokio_postgres::Client,
  verification_challenge: VerificationChallenge,
) -> Result<response::VerificationChallenge, response::AuthError> {
  Ok(response::VerificationChallenge {
    creation_time: verification_challenge.creation_time,
    to_parent: verification_challenge.to_parent,
    email: verification_challenge.email,
  })
}

pub async fn get_api_key_if_valid_noverify(
  con: &mut tokio_postgres::Client,
  api_key: &str,
) -> Result<ApiKey, response::AuthError> {
  let creator_api_key = api_key_service::get_by_api_key_hash(con, &utils::hash_str(api_key))
    .await
    .map_err(report_postgres_err)?
    .ok_or(response::AuthError::ApiKeyNonexistent)?;

  if utils::current_time_millis() > creator_api_key.creation_time + creator_api_key.duration {
    return Err(response::AuthError::ApiKeyUnauthorized);
  }

  Ok(creator_api_key)
}

pub async fn get_api_key_if_verified(
  con: &mut tokio_postgres::Client,
  api_key: &str,
) -> Result<ApiKey, response::AuthError> {
  let creator_api_key = get_api_key_if_valid_noverify(con, api_key).await?;

  // ensure parent permission
  let _ = parent_permission_service::get_by_user_id(con, creator_api_key.creator_user_id)
    .await
    .map_err(report_postgres_err)?
    .ok_or(response::AuthError::ParentPermissionNonexistent)?;

  Ok(creator_api_key)
}

pub async fn api_key_new_valid(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::ApiKeyNewValidProps,
) -> Result<response::ApiKey, response::AuthError> {
  let con = &mut *db.lock().await;

  let email = email_service::get_by_email(con, &props.user_email)
    .await
    .map_err(report_postgres_err)?
    .ok_or(response::AuthError::EmailNonexistent)?;

  let password = password_service::get_by_user_id(con, email.creator_user_id)
    .await
    .map_err(report_postgres_err)?
    .ok_or(response::AuthError::PasswordNonexistent)?;

  // validate password with bcrypt
  if !utils::verify_password(&props.user_password, &password.password_hash)
    .map_err(report_internal_err)?
  {
    return Err(response::AuthError::PasswordIncorrect);
  }

  // TODO verify parental permission here

  let raw_api_key = utils::gen_random_string();

  let mut sp = con.transaction().await.map_err(report_postgres_err)?;

  // add new api key
  let api_key = api_key_service::add(
    &mut sp,
    email.creator_user_id,
    utils::hash_str(&raw_api_key),
    request::ApiKeyKind::Valid,
    props.duration,
  )
  .await
  .map_err(report_postgres_err)?;

  sp.commit().await.map_err(report_postgres_err)?;

  fill_api_key(con, api_key, Some(raw_api_key)).await
}

pub async fn api_key_new_cancel(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::ApiKeyNewCancelProps,
) -> Result<response::ApiKey, response::AuthError> {
  let con = &mut *db.lock().await;

  // validate api key
  let creator_key = get_api_key_if_verified(con, &props.api_key).await?;

  let to_cancel_key = get_api_key_if_verified(con, &props.api_key_to_cancel).await?;

  if creator_key.creator_user_id != to_cancel_key.creator_user_id {
    return Err(response::AuthError::ApiKeyUnauthorized);
  }

  let mut sp = con.transaction().await.map_err(report_postgres_err)?;

  // cancel keys
  let key_cancel = api_key_service::add(
    &mut sp,
    creator_key.creator_user_id,
    to_cancel_key.api_key_hash,
    request::ApiKeyKind::Cancel,
    0,
  )
  .await
  .map_err(report_postgres_err)?;

  sp.commit().await.map_err(report_postgres_err)?;

  // return json
  fill_api_key(con, key_cancel, None).await
}

pub async fn send_parent_permission_email(
  mail_service: &MailService,
  target_email: &str,
  user_name: &str,
  site_external_url: &str,
  verification_challenge_key: &str,
) -> Result<(), response::AuthError> {
  let _ = mail_service
    .mail_new(mail_service_api::request::MailNewProps {
      request_id: 0,
      destination: target_email.to_owned(),
      topic: "parent_permission".to_owned(),
      title: format!("{}: Parent Permission For {}", site_external_url, user_name),
      content: [
        &format!(
          "<p>Your child, <code>{}</code>, has requested permission to use: <code>{}</code></p>",
          user_name, site_external_url
        ),
        "<p>If you did not make this request, then feel free to ignore.</p>",
        "<p>This link is valid for up to 15 minutes.</p>",
        "<p>Do not share this link with others.</p>",
        &format!(
          "<p>Verification link: {}/parent_confirm?verificationChallengeKey={}</p>",
          site_external_url, verification_challenge_key
        ),
      ]
      .join(""),
    })
    .await
    .map_err(report_mail_err)?;

  Ok(())
}

pub async fn send_email_verification_email(
  mail_service: &MailService,
  target_email: &str,
  user_name: &str,
  site_external_url: &str,
  verification_challenge_key: &str,
) -> Result<(), response::AuthError> {
  let _ = mail_service
    .mail_new(mail_service_api::request::MailNewProps {
      request_id: 0,
      destination: target_email.to_owned(),
      topic: "verification_challenge".to_owned(),
      title: format!("{}: Email Verification", site_external_url),
      content: [
        &format!(
          "<p>This email has been sent to verify for: <code>{}</code> </p>",
          &user_name
        ),
        "<p>If you did not make this request, then feel free to ignore.</p>",
        "<p>This link is valid for up to 15 minutes.</p>",
        "<p>Do not share this link with others.</p>",
        &format!(
          "<p>Verification link: {}/email_confirm?verificationChallengeKey={}</p>",
          site_external_url, verification_challenge_key
        ),
      ]
      .join(""),
    })
    .await
    .map_err(report_mail_err)?;
  Ok(())
}

pub async fn verification_challenge_new(
  config: Config,
  db: Db,
  mail_service: MailService,
  props: request::VerificationChallengeNewProps,
) -> Result<response::VerificationChallenge, response::AuthError> {
  // avoid sending email to obviously bad addresses
  if props.email.is_empty() {
    return Err(response::AuthError::EmailBounced);
  }

  let con = &mut *db.lock().await;
  // api key verification required
  let api_key = get_api_key_if_valid_noverify(con, &props.api_key).await?;

  // if to_parent, check that parental permission doesn't exist yet
  if props.to_parent
    && parent_permission_service::get_by_user_id(con, api_key.creator_user_id)
      .await
      .map_err(report_postgres_err)?
      .is_some()
  {
    return Err(response::AuthError::ParentPermissionExistent);
  }

  // don't let people spam emails
  let last_email_sent_time =
    verification_challenge_service::get_latest_time_for_creator(con, api_key.creator_user_id)
      .await
      .map_err(report_postgres_err)?;

  if let Some(time) = last_email_sent_time {
    if time + FIFTEEN_MINUTES as i64 > utils::current_time_millis() {
      // TOOD more descriptive error
      return Err(response::AuthError::EmailUnknown);
    }
  }

  // get user data to generate
  let user_data = user_data_service::get_by_user_id(con, api_key.creator_user_id)
    .await
    .map_err(report_postgres_err)?
    .ok_or(response::AuthError::UserDataNonexistent)?;

  // generate random string
  let verification_challenge_key = utils::gen_random_string();

  // send email depending on kind
  if props.to_parent {
    send_parent_permission_email(
      &mail_service,
      &props.email,
      &user_data.name,
      &config.site_external_url,
      &verification_challenge_key,
    )
    .await?;
  } else {
    send_email_verification_email(
      &mail_service,
      &props.email,
      &user_data.name,
      &config.site_external_url,
      &verification_challenge_key,
    )
    .await?;
  }

  // insert into database
  let verification_challenge = verification_challenge_service::add(
    con,
    utils::hash_str(&verification_challenge_key),
    props.email,
    api_key.creator_user_id,
    props.to_parent,
  )
  .await
  .map_err(report_postgres_err)?;

  // return json
  fill_verification_challenge(con, verification_challenge).await
}

pub async fn user_new(
  config: Config,
  db: Db,
  mail_service: MailService,
  props: request::UserNewProps,
) -> Result<response::UserData, response::AuthError> {
  // name isn't empty
  if props.user_name.is_empty() {
    return Err(response::AuthError::UserNameEmpty);
  }

  // server side validation of password strength
  if !utils::is_secure_password(&props.user_password) {
    return Err(response::AuthError::PasswordInsecure);
  }

  let con = &mut *db.lock().await;

  let mut sp = con.transaction().await.map_err(report_postgres_err)?;

  // create user
  let user = user_service::add(&mut sp)
    .await
    .map_err(report_postgres_err)?;

  // create user data
  let user_data = user_data_service::add(&mut sp, user.user_id, props.user_name)
    .await
    .map_err(report_postgres_err)?;

  // create password
  let password_hash = utils::hash_password(&props.user_password).map_err(report_internal_err)?;
  password_service::add(&mut sp, user.user_id, password_hash, None)
    .await
    .map_err(report_postgres_err)?;

  // create email request
  let verification_challenge_key = utils::gen_random_string();

  send_email_verification_email(
    &mail_service,
    &props.user_email,
    &user_data.name,
    &config.site_external_url,
    &verification_challenge_key,
  )
  .await?;

  // insert into database
  verification_challenge_service::add(
    &mut sp,
    utils::hash_str(&verification_challenge_key),
    props.user_email,
    user.user_id,
    false,
  )
  .await
  .map_err(report_postgres_err)?;

  // if it was indicated that the user is under than 13, send a parental permission
  if let Some(parent_email) = props.parent_email {
    // add new verification challenge for parent
    let verification_challenge_key = utils::gen_random_string();

    // create email request
    send_parent_permission_email(
      &mail_service,
      &parent_email,
      &user_data.name,
      &config.site_external_url,
      &verification_challenge_key,
    )
    .await?;

    // insert into database
    verification_challenge_service::add(
      &mut sp,
      utils::hash_str(&verification_challenge_key),
      parent_email,
      user.user_id,
      true,
    )
    .await
    .map_err(report_postgres_err)?;
  } else {
    // the user says they are over 13
    parent_permission_service::add(&mut sp, user.user_id, None)
      .await
      .map_err(report_postgres_err)?;
  }

  sp.commit().await.map_err(report_postgres_err)?;

  // return filled struct
  fill_user_data(con, user_data).await
}

pub async fn user_data_new(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::UserDataNewProps,
) -> Result<response::UserData, response::AuthError> {
  let con = &mut *db.lock().await;

  // api key verification required (email or parent permission not needed)
  let creator_key = get_api_key_if_valid_noverify(con, &props.api_key).await?;

  // create key data
  let user_data = user_data_service::add(con, creator_key.creator_user_id, props.user_name)
    .await
    .map_err(report_postgres_err)?;

  // return json
  fill_user_data(con, user_data).await
}

pub async fn email_new(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::EmailNewProps,
) -> Result<response::Email, response::AuthError> {
  let con = &mut *db.lock().await;

  let vckh = utils::hash_str(&props.verification_challenge_key);

  // check that the verification challenge exists
  let vc = verification_challenge_service::get_by_verification_challenge_key_hash(con, &vckh)
    .await
    .map_err(report_postgres_err)?
    .ok_or(response::AuthError::VerificationChallengeNonexistent)?;

  // check that it hasn't timed out
  if FIFTEEN_MINUTES as i64 + vc.creation_time < utils::current_time_millis() {
    return Err(response::AuthError::VerificationChallengeTimedOut);
  }

  // check that the verification challenge is meant for the correct purpose
  if vc.to_parent {
    return Err(response::AuthError::VerificationChallengeWrongKind);
  }

  // check if the verification challenge was not already used to make a new email
  if email_service::get_by_verification_challenge_key_hash(con, &vckh)
    .await
    .map_err(report_postgres_err)?
    .is_some()
  {
    return Err(response::AuthError::VerificationChallengeUsed);
  }

  // check that the email isn't already in use by another user
  if email_service::get_by_email(con, &vc.email)
    .await
    .map_err(report_postgres_err)?
    .is_some()
  {
    return Err(response::AuthError::EmailExistent);
  }

  // create key data
  let email = email_service::add(con, vc.creator_user_id, vckh)
    .await
    .map_err(report_postgres_err)?;

  // return json
  fill_email(con, email).await
}

pub async fn parent_permission_new(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::ParentPermissionNewProps,
) -> Result<response::ParentPermission, response::AuthError> {
  let con = &mut *db.lock().await;

  let vckh = utils::hash_str(&props.verification_challenge_key);

  // check that the verification challenge exists
  let vc = verification_challenge_service::get_by_verification_challenge_key_hash(con, &vckh)
    .await
    .map_err(report_postgres_err)?
    .ok_or(response::AuthError::VerificationChallengeNonexistent)?;

  // check that it hasn't timed out
  if FIFTEEN_MINUTES as i64 + vc.creation_time < utils::current_time_millis() {
    return Err(response::AuthError::VerificationChallengeTimedOut);
  }

  // check that the verification challenge is meant for the correct purpose
  if !vc.to_parent {
    return Err(response::AuthError::VerificationChallengeWrongKind);
  }

  // check if the verification challenge was not already used to make a new parent_permission
  if parent_permission_service::get_by_verification_challenge_key_hash(con, &vckh)
    .await
    .map_err(report_postgres_err)?
    .is_some()
  {
    return Err(response::AuthError::VerificationChallengeUsed);
  }

  // create key data
  let parent_permission = parent_permission_service::add(con, vc.creator_user_id, Some(vckh))
    .await
    .map_err(report_postgres_err)?;

  // return json
  fill_parent_permission(con, parent_permission).await
}

pub async fn password_reset_new(
  config: Config,
  db: Db,
  mail_service: MailService,
  props: request::PasswordResetNewProps,
) -> Result<response::PasswordReset, response::AuthError> {
  let con = &mut *db.lock().await;

  let email = email_service::get_by_email(con, &props.user_email)
    .await
    .map_err(report_postgres_err)?
    .ok_or(response::AuthError::EmailNonexistent)?;

  let raw_key = utils::gen_random_string();

  // send mail
  let _ = mail_service
    .mail_new(mail_service_api::request::MailNewProps {
      request_id: 0,
      destination: props.user_email,
      topic: "password_reset".to_owned(),
      title: format!("{}: Password Reset", &config.site_external_url),
      content: [
        "<p>Requested password reset service: </p>",
        "<p>If you did not make this request, then feel free to ignore.</p>",
        "<p>This link is valid for up to 15 minutes.</p>",
        "<p>Do not share this link with others.</p>",
        &format!(
          "<p>Password change link: {}/reset_password?resetKey={}</p>",
          &config.site_external_url, raw_key
        ),
      ]
      .join(""),
    })
    .await
    .map_err(report_mail_err)?;

  let mut sp = con.transaction().await.map_err(report_postgres_err)?;

  let password_reset =
    password_reset_service::add(&mut sp, utils::hash_str(&raw_key), email.creator_user_id)
      .await
      .map_err(report_postgres_err)?;

  sp.commit().await.map_err(report_postgres_err)?;

  // fill struct
  fill_password_reset(con, password_reset).await
}

pub async fn password_new_reset(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::PasswordNewResetProps,
) -> Result<response::Password, response::AuthError> {
  // no api key verification needed

  let con = &mut *db.lock().await;

  // get password reset
  let psr = password_reset_service::get_by_password_reset_key_hash(
    con,
    &utils::hash_str(&props.password_reset_key),
  )
  .await
  .map_err(report_postgres_err)?
  .ok_or(response::AuthError::PasswordResetNonexistent)?;

  // deny if we alread created a password from this reset
  if password_service::exists_by_password_reset_key_hash(con, &psr.password_reset_key_hash)
    .await
    .map_err(report_postgres_err)?
  {
    return Err(response::AuthError::PasswordExistent);
  }

  // deny if timed out
  if FIFTEEN_MINUTES as i64 + psr.creation_time < utils::current_time_millis() {
    return Err(response::AuthError::PasswordResetTimedOut);
  }

  // reject insecure passwords
  if !utils::is_secure_password(&props.new_password) {
    return Err(response::AuthError::PasswordInsecure);
  }

  // attempt to hash password
  let new_password_hash = utils::hash_password(&props.new_password).map_err(report_internal_err)?;

  let mut sp = con.transaction().await.map_err(report_postgres_err)?;

  // create password
  let password = password_service::add(
    &mut sp,
    psr.creator_user_id,
    new_password_hash,
    Some(psr.password_reset_key_hash),
  )
  .await
  .map_err(report_postgres_err)?;

  sp.commit().await.map_err(report_postgres_err)?;

  fill_password(con, password).await
}

pub async fn password_new_change(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::PasswordNewChangeProps,
) -> Result<response::Password, response::AuthError> {
  let con = &mut *db.lock().await;

  // api key verification required (no parent permission needed tho)
  let creator_key = get_api_key_if_valid_noverify(con, &props.api_key).await?;

  // reject insecure passwords
  if !utils::is_secure_password(&props.new_password) {
    return Err(response::AuthError::PasswordInsecure);
  }

  // attempt to hash password
  let new_password_hash = utils::hash_password(&props.new_password).map_err(report_internal_err)?;

  let mut sp = con.transaction().await.map_err(report_postgres_err)?;

  // create password
  let password = password_service::add(
    &mut sp,
    creator_key.creator_user_id,
    new_password_hash,
    None,
  )
  .await
  .map_err(report_postgres_err)?;

  sp.commit().await.map_err(report_postgres_err)?;

  // return filled struct
  fill_password(con, password).await
}

pub async fn user_view(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::UserViewProps,
) -> Result<Vec<response::User>, response::AuthError> {
  let con = &mut *db.lock().await;
  // api key verification required
  let _ = get_api_key_if_valid_noverify(con, &props.api_key).await?;
  // get users
  let users = user_service::query(con, props)
    .await
    .map_err(report_postgres_err)?;

  let mut resp_users = vec![];
  for u in users.into_iter() {
    resp_users.push(fill_user(con, u).await?);
  }

  Ok(resp_users)
}

pub async fn user_data_view(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::UserDataViewProps,
) -> Result<Vec<response::UserData>, response::AuthError> {
  let con = &mut *db.lock().await;
  // api key verification required
  let _ = get_api_key_if_valid_noverify(con, &props.api_key).await?;
  // get user_datas
  let user_datas = user_data_service::query(con, props)
    .await
    .map_err(report_postgres_err)?;

  let mut resp_user_datas = vec![];
  for u in user_datas.into_iter() {
    resp_user_datas.push(fill_user_data(con, u).await?);
  }

  Ok(resp_user_datas)
}

pub async fn email_view(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::EmailViewProps,
) -> Result<Vec<response::Email>, response::AuthError> {
  let con = &mut *db.lock().await;
  // api key verification required
  let _ = get_api_key_if_valid_noverify(con, &props.api_key).await?;
  // get emails
  let emails = email_service::query(con, props)
    .await
    .map_err(report_postgres_err)?;

  // return emails
  let mut resp_emails = vec![];
  for u in emails.into_iter() {
    resp_emails.push(fill_email(con, u).await?);
  }

  Ok(resp_emails)
}

pub async fn password_view(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::PasswordViewProps,
) -> Result<Vec<response::Password>, response::AuthError> {
  let con = &mut *db.lock().await;
  // api key verification required
  let _ = get_api_key_if_valid_noverify(con, &props.api_key).await?;
  // get passwords
  let passwords = password_service::query(con, props)
    .await
    .map_err(report_postgres_err)?;

  // return passwords
  let mut resp_passwords = vec![];
  for u in passwords.into_iter() {
    resp_passwords.push(fill_password(con, u).await?);
  }

  Ok(resp_passwords)
}

pub async fn parent_permission_view(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::ParentPermissionViewProps,
) -> Result<Vec<response::ParentPermission>, response::AuthError> {
  let con = &mut *db.lock().await;
  // api key verification required
  let _ = get_api_key_if_valid_noverify(con, &props.api_key).await?;
  // get parent_permissions
  let parent_permissions = parent_permission_service::query(con, props)
    .await
    .map_err(report_postgres_err)?;

  let mut resp_parent_permissions = vec![];
  for u in parent_permissions.into_iter() {
    resp_parent_permissions.push(fill_parent_permission(con, u).await?);
  }

  Ok(resp_parent_permissions)
}

pub async fn verification_challenge_view(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::VerificationChallengeViewProps,
) -> Result<Vec<response::VerificationChallenge>, response::AuthError> {
  let con = &mut *db.lock().await;
  // api key verification required
  let _ = get_api_key_if_valid_noverify(con, &props.api_key).await?;
  // get verification_challenges
  let verification_challenges = verification_challenge_service::query(con, props)
    .await
    .map_err(report_postgres_err)?;

  let mut resp_verification_challenges = vec![];
  for u in verification_challenges.into_iter() {
    resp_verification_challenges.push(fill_verification_challenge(con, u).await?);
  }

  Ok(resp_verification_challenges)
}

pub async fn api_key_view(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::ApiKeyViewProps,
) -> Result<Vec<response::ApiKey>, response::AuthError> {
  let con = &mut *db.lock().await;
  // api key verification required
  let _ = get_api_key_if_valid_noverify(con, &props.api_key).await?;
  // get users
  let api_keys = api_key_service::query(con, props)
    .await
    .map_err(report_postgres_err)?;

  // return
  let mut resp_api_keys = vec![];
  for u in api_keys.into_iter() {
    resp_api_keys.push(fill_api_key(con, u, None).await?);
  }

  Ok(resp_api_keys)
}

// special internal api
pub async fn get_user_by_id(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::GetUserByIdProps,
) -> Result<response::User, response::AuthError> {
  let con = &mut *db.lock().await;

  let user = user_service::get_by_user_id(con, props.user_id)
    .await
    .map_err(report_postgres_err)?
    .ok_or(response::AuthError::UserNonexistent)?;

  fill_user(con, user).await
}

pub async fn get_user_by_api_key_if_valid(
  _config: Config,
  db: Db,
  _mail_service: MailService,
  props: request::GetUserByApiKeyIfValid,
) -> Result<response::User, response::AuthError> {
  let con = &mut *db.lock().await;

  let api_key = get_api_key_if_verified(con, &props.api_key).await?;

  let user = user_service::get_by_user_id(con, api_key.creator_user_id)
    .await
    .map_err(report_postgres_err)?
    .ok_or(response::AuthError::UserNonexistent)?;

  fill_user(con, user).await
}
